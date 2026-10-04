use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, anyhow, bail};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::driver::{
    DriverType, Environment, EnvironmentMetadata, EnvironmentPurpose, Persistence,
};
use debmagic_common::distro::{Distro, DistroVersion};

/// Sequential schema steps, compiled from `migrations/NNNN.sql`.
/// `PRAGMA user_version` is the count of applied steps.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001.sql"),
    include_str!("../migrations/0002.sql"),
];

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Switch to WAL, retrying on SQLITE_BUSY. Changing the journal mode upgrades
/// a shared lock, and SQLite reports that conflict immediately instead of
/// invoking the busy handler, so concurrent first opens must retry by hand.
fn enable_wal(conn: &Connection) -> anyhow::Result<()> {
    let deadline = Instant::now() + BUSY_TIMEOUT;
    loop {
        match conn.pragma_update(None, "journal_mode", "WAL") {
            Ok(()) => return Ok(()),
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == rusqlite::ErrorCode::DatabaseBusy && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                return Err(error).context("enabling WAL on the Environment registry failed");
            }
        }
    }
}

#[derive(Debug)]
struct IncompatibleSchema {
    found: i64,
    supported: i64,
}

impl std::fmt::Display for IncompatibleSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Environment registry schema version {} is newer than this debmagic ({}); upgrade debmagic",
            self.found, self.supported
        )
    }
}

impl std::error::Error for IncompatibleSchema {}

fn user_version(conn: &Connection) -> anyhow::Result<i64> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .context("reading Environment registry schema version failed")
}

fn is_incompatible_schema(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.downcast_ref::<IncompatibleSchema>().is_some())
}

fn is_busy(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<rusqlite::Error>(),
            Some(rusqlite::Error::SqliteFailure(failure, _))
                if matches!(
                    failure.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                )
        )
    })
}

/// Apply each not-yet-run step in its own IMMEDIATE transaction, stamping
/// `user_version` in the same transaction so a crash cannot leave a half-applied step.
///
/// The version is re-read under the write lock: another process may have
/// applied the step between our first read and acquiring the lock.
fn migrate_with(conn: &mut Connection, migrations: &[&str]) -> anyhow::Result<()> {
    let supported = migrations.len() as i64;
    let ensure_supported = |current: i64| -> anyhow::Result<()> {
        if current > supported {
            return Err(IncompatibleSchema {
                found: current,
                supported,
            }
            .into());
        }
        Ok(())
    };
    let current = user_version(conn)?;
    ensure_supported(current)?;
    if current == supported {
        return Ok(());
    }
    for (index, step) in migrations.iter().enumerate() {
        let next = (index + 1) as i64;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = user_version(&tx)?;
        ensure_supported(current)?;
        if current >= next {
            continue;
        }
        tx.execute_batch(step).with_context(|| {
            format!(
                "migrating the Environment registry from schema version {} to {next} failed",
                next - 1
            )
        })?;
        tx.pragma_update(None, "user_version", next)
            .context("writing Environment registry schema version failed")?;
        tx.commit()?;
    }
    Ok(())
}

fn migrate(conn: &mut Connection) -> anyhow::Result<()> {
    migrate_with(conn, MIGRATIONS)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationKind {
    BinaryBuild,
    SourceBuild,
    TestRun,
}

impl InvocationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::BinaryBuild => "binary_build",
            Self::SourceBuild => "source_build",
            Self::TestRun => "test_run",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecordedInvocation {
    pub kind: InvocationKind,
    pub source_dir: PathBuf,
    pub package_version: String,
    pub distro: DistroVersion,
    pub driver: Option<DriverType>,
    pub success: bool,
    pub changes_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RegisteredEnvironment {
    pub environment: Environment,
    pub(super) driver_metadata: HashMap<String, String>,
    pub(super) owner_pid: Option<u32>,
    /// Start time of `owner_pid` (from `/proc`), used to detect PID reuse.
    pub(super) owner_pid_start: Option<i64>,
}

impl RegisteredEnvironment {
    pub(super) fn metadata(&self) -> EnvironmentMetadata {
        EnvironmentMetadata {
            environment: self.environment.clone(),
            driver_metadata: self.driver_metadata.clone(),
        }
    }
}

pub struct Registry {
    conn: Connection,
}

impl Registry {
    fn from_connection(mut conn: Connection) -> anyhow::Result<Self> {
        conn.pragma_update(None, "busy_timeout", BUSY_TIMEOUT.as_millis() as i64)?;
        enable_wal(&conn)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {} failed", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening environment registry {} failed", path.display()))?;
        Self::from_connection(conn)
            .with_context(|| format!("opening environment registry {} failed", path.display()))
    }

    pub fn open_default() -> anyhow::Result<Self> {
        Self::open(&crate::data_dir::registry_path()?)
    }

    /// Open `path` when the file already exists. Does not create the file or
    /// its parent directory.
    pub fn open_if_exists(path: &Path) -> anyhow::Result<Option<Self>> {
        if !path.is_file() {
            return Ok(None);
        }
        Self::open(path).map(Some)
    }

    /// Open the on-disk registry when it already exists. `debmagic env` reads
    /// use this so listing or cleaning does not create an empty database.
    pub fn open_default_if_exists() -> anyhow::Result<Option<Self>> {
        Self::open_if_exists(&crate::data_dir::registry_path()?)
    }

    fn open_memory() -> anyhow::Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn warn_and_open_memory(error: &anyhow::Error) -> Self {
        eprintln!(
            "Warning: cannot open the Environment registry ({error}); \
             continuing without persistent bookkeeping"
        );
        Self::open_memory().expect("an in-memory registry cannot fail to open")
    }

    /// Open `path`, or a process-local in-memory registry when the file cannot
    /// be used. Still an error:
    /// - a schema newer than this binary — falling back would hide
    ///   Environments the newer debmagic already recorded;
    /// - a registry that is busy or locked — it works, another debmagic holds
    ///   it, and bypassing it would skip the "already in use" check that keeps
    ///   two runs off the same Environment.
    fn open_or_ephemeral(path: &Path) -> anyhow::Result<Self> {
        match Self::open(path) {
            Ok(registry) => Ok(registry),
            Err(error) if is_incompatible_schema(&error) || is_busy(&error) => Err(error),
            Err(error) => Ok(Self::warn_and_open_memory(&error)),
        }
    }

    /// For build/test bookkeeping: fall back to a process-local in-memory
    /// registry when the on-disk one cannot be opened, so bookkeeping never
    /// fails the actual work — except a schema newer than this binary or a
    /// busy registry, which are still errors (see [`Self::open_or_ephemeral`]). `debmagic env` commands must use [`Self::open_default`]
    /// instead — they *are* registry operations.
    pub fn open_default_or_ephemeral() -> anyhow::Result<Self> {
        match crate::data_dir::registry_path() {
            Ok(path) => Self::open_or_ephemeral(&path),
            Err(error) => Ok(Self::warn_and_open_memory(&error)),
        }
    }

    pub(super) fn upsert_environment(
        &self,
        environment: &Environment,
        owner_pid: Option<u32>,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO environments (
                id, driver, package_name, package_identifier, source_dir, root_dir,
                distro_family, distro_codename, distro_version, distro_is_devel,
                persistent, purpose, owner_pid, owner_pid_start, destroying, driver_metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0, '{}')
            ON CONFLICT(id) DO UPDATE SET
                driver = excluded.driver,
                package_name = excluded.package_name,
                package_identifier = excluded.package_identifier,
                source_dir = excluded.source_dir,
                root_dir = excluded.root_dir,
                distro_family = excluded.distro_family,
                distro_codename = excluded.distro_codename,
                distro_version = excluded.distro_version,
                distro_is_devel = excluded.distro_is_devel,
                persistent = excluded.persistent,
                purpose = excluded.purpose,
                owner_pid = excluded.owner_pid,
                owner_pid_start = excluded.owner_pid_start,
                destroying = 0",
            params![
                environment.id(),
                environment.driver.as_str(),
                environment.package_name,
                environment.package_identifier,
                path_to_string(&environment.source_dir),
                path_to_string(&environment.root_dir),
                environment.distro.distro.as_str(),
                environment.distro.codename,
                environment.distro.version,
                environment.distro.is_devel as i64,
                environment.persistence.as_i64(),
                environment.purpose.as_str(),
                owner_pid.map(|pid| pid as i64),
                owner_pid.and_then(|pid| pid_start_time(pid).map(|start| start as i64)),
            ],
        )?;
        Ok(())
    }

    /// Register `environment` with `owner_pid` as its owner and record the
    /// Driver metadata planned for it, refusing when a *live* different
    /// process already owns it or a live Attachment is open on it (the run
    /// would reset the Host root, or replace the container, under the shell).
    /// Atomic (BEGIN IMMEDIATE), so concurrent commands cannot both claim the
    /// same Environment.
    ///
    /// Returns the row as it was before, so the caller can still reach a
    /// Driver resource recorded under the previous metadata.
    pub(super) fn begin_environment(
        &self,
        environment: &Environment,
        owner_pid: u32,
        planned_driver_metadata: &HashMap<String, String>,
    ) -> anyhow::Result<Option<RegisteredEnvironment>> {
        let id = environment.id();
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| -> anyhow::Result<Option<RegisteredEnvironment>> {
            let previous = self.get(&id)?;
            if let Some(pid) = previous.as_ref().and_then(|row| row.owner_pid)
                && pid != owner_pid
                && pid_matches(pid, previous.as_ref().and_then(|row| row.owner_pid_start))
            {
                bail!("Environment {id} is already in use by process {pid}");
            }
            if previous.is_some() {
                self.reap_dead_attachments(&id)?;
                let attachments = self.attachment_count(&id)?;
                if attachments > 0 {
                    bail!(
                        "Environment {id} has {attachments} live Attachment(s) \
                         (`debmagic env shell`); exit them first"
                    );
                }
            }
            self.upsert_environment(environment, Some(owner_pid))?;
            self.set_driver_metadata(&id, planned_driver_metadata)?;
            Ok(previous)
        })();
        match &result {
            Ok(_) => self.conn.execute_batch("COMMIT")?,
            Err(_) => {
                let _ = self.conn.execute_batch("ROLLBACK");
            }
        }
        result
    }

    pub(super) fn set_driver_metadata(
        &self,
        environment_id: &str,
        metadata: &HashMap<String, String>,
    ) -> anyhow::Result<()> {
        let json = serde_json::to_string(metadata)?;
        let updated = self.conn.execute(
            "UPDATE environments SET driver_metadata = ?1 WHERE id = ?2",
            params![json, environment_id],
        )?;
        if updated == 0 {
            bail!("environment {environment_id} is not in the registry");
        }
        Ok(())
    }

    /// Clear the owner, but only if `owner_pid` still is the owner.
    pub(super) fn clear_owner(&self, environment_id: &str, owner_pid: u32) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE environments SET owner_pid = NULL, owner_pid_start = NULL
             WHERE id = ?1 AND owner_pid = ?2",
            params![environment_id, owner_pid as i64],
        )?;
        Ok(())
    }

    /// Atomically verify that an Environment has no live Attachments and no
    /// live owner, then mark it as being destroyed and claim ownership for
    /// this process. Blocks concurrent `env shell` Attachments (which check
    /// `destroying` on insert) and concurrent `begin_environment` calls (which
    /// see a live owner) for the duration of the destroy.
    pub(super) fn claim_for_destroy(&self, environment_id: &str) -> anyhow::Result<()> {
        let self_pid = current_pid();
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| -> anyhow::Result<()> {
            self.reap_dead_attachments(environment_id)?;
            let row = self
                .conn
                .query_row(
                    "SELECT destroying, owner_pid, owner_pid_start,
                            (SELECT COUNT(*) FROM attachments WHERE environment_id = ?1)
                     FROM environments WHERE id = ?1",
                    params![environment_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)? != 0,
                            row.get::<_, Option<i64>>(1)?,
                            row.get::<_, Option<i64>>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
                .optional()?
                .with_context(|| format!("no Environment with id {environment_id}"))?;
            let (destroying, owner_pid, owner_pid_start, attachments) = row;
            if attachments > 0 {
                bail!("Environment {environment_id} has live Attachments; stop them first");
            }
            if let Some(pid) = owner_pid.map(|pid| pid as u32)
                && pid != self_pid
                && pid_matches(pid, owner_pid_start)
            {
                if destroying {
                    bail!(
                        "Environment {environment_id} is already being destroyed by process {pid}"
                    );
                }
                bail!("Environment {environment_id} still has a creating command running");
            }
            self.conn.execute(
                "UPDATE environments SET destroying = 1, owner_pid = ?2, owner_pid_start = ?3
                 WHERE id = ?1",
                params![
                    environment_id,
                    self_pid as i64,
                    pid_start_time(self_pid).map(|start| start as i64)
                ],
            )?;
            Ok(())
        })();
        match &result {
            Ok(()) => self.conn.execute_batch("COMMIT")?,
            Err(_) => {
                let _ = self.conn.execute_batch("ROLLBACK");
            }
        }
        result
    }

    /// Wait until no live Attachments remain, then atomically mark the
    /// Environment as destroying so no new Attachments can appear. Guards on
    /// `owner_pid` still owning the row.
    pub(super) fn wait_and_mark_destroying(
        &self,
        environment_id: &str,
        owner_pid: u32,
    ) -> anyhow::Result<()> {
        let mut announced = false;
        loop {
            self.conn.execute_batch("BEGIN IMMEDIATE")?;
            let result = (|| -> anyhow::Result<bool> {
                self.reap_dead_attachments(environment_id)?;
                let count: i64 = self.conn.query_row(
                    "SELECT COUNT(*) FROM attachments WHERE environment_id = ?1",
                    params![environment_id],
                    |row| row.get(0),
                )?;
                if count > 0 {
                    return Ok(false);
                }
                let updated = self.conn.execute(
                    "UPDATE environments SET destroying = 1 WHERE id = ?1 AND owner_pid = ?2",
                    params![environment_id, owner_pid as i64],
                )?;
                if updated == 0 {
                    bail!("Environment {environment_id} is no longer owned by this process");
                }
                Ok(true)
            })();
            match result {
                Ok(true) => {
                    self.conn.execute_batch("COMMIT")?;
                    return Ok(());
                }
                Ok(false) => {
                    self.conn.execute_batch("COMMIT")?;
                    if !announced {
                        println!("Waiting for all attached shells to exit...");
                        announced = true;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                Err(error) => {
                    let _ = self.conn.execute_batch("ROLLBACK");
                    return Err(error);
                }
            }
        }
    }

    /// Undo [`Self::claim_for_destroy`] without destroying, but only if
    /// `owner_pid` still owns the row.
    pub(super) fn release_destroy_claim(
        &self,
        environment_id: &str,
        owner_pid: u32,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE environments SET destroying = 0, owner_pid = NULL, owner_pid_start = NULL
             WHERE id = ?1 AND owner_pid = ?2",
            params![environment_id, owner_pid as i64],
        )?;
        Ok(())
    }

    /// Delete the registry row, but only if `owner_pid` still owns it.
    pub(super) fn finish_destroy(
        &self,
        environment_id: &str,
        owner_pid: u32,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM environments WHERE id = ?1 AND owner_pid = ?2",
            params![environment_id, owner_pid as i64],
        )?;
        Ok(())
    }

    pub fn get(&self, environment_id: &str) -> anyhow::Result<Option<RegisteredEnvironment>> {
        self.conn
            .query_row(
                &format!("SELECT {ENVIRONMENT_COLUMNS} FROM environments WHERE id = ?1"),
                params![environment_id],
                row_to_registered,
            )
            .optional()
            .context("reading environment from the registry failed")
    }

    pub fn list(&self) -> anyhow::Result<Vec<RegisteredEnvironment>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ENVIRONMENT_COLUMNS} FROM environments
             ORDER BY package_name, source_dir, purpose, driver"
        ))?;
        let rows = stmt.query_map([], row_to_registered)?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("listing environments failed")
    }

    pub fn list_for_source_tree(
        &self,
        source_dir: &Path,
    ) -> anyhow::Result<Vec<RegisteredEnvironment>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ENVIRONMENT_COLUMNS} FROM environments WHERE source_dir = ?1
             ORDER BY purpose, driver, distro_codename"
        ))?;
        let rows = stmt.query_map(params![path_to_string(source_dir)], row_to_registered)?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("listing environments for source tree failed")
    }

    /// Attach `pid` to an Environment. Fails when the Environment is gone or
    /// being destroyed; the existence/`destroying` check and the insert are a
    /// single statement, so this cannot race with `claim_for_destroy`.
    pub(super) fn add_attachment(&self, environment_id: &str, pid: u32) -> anyhow::Result<i64> {
        let inserted = self.conn.execute(
            "INSERT INTO attachments (environment_id, pid, pid_start)
             SELECT ?1, ?2, ?3
             WHERE EXISTS (
                 SELECT 1 FROM environments WHERE id = ?1 AND destroying = 0
             )",
            params![
                environment_id,
                pid as i64,
                pid_start_time(pid).map(|start| start as i64)
            ],
        )?;
        if inserted == 0 {
            bail!("Environment {environment_id} is gone or being destroyed");
        }
        Ok(self.conn.last_insert_rowid())
    }

    pub(super) fn remove_attachment(&self, attachment_id: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM attachments WHERE id = ?1",
            params![attachment_id],
        )?;
        Ok(())
    }

    pub(super) fn reap_dead_attachments(&self, environment_id: &str) -> anyhow::Result<usize> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, pid, pid_start FROM attachments WHERE environment_id = ?1")?;
        let rows = stmt.query_map(params![environment_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)? as u32,
                row.get::<_, Option<i64>>(2)?,
            ))
        })?;
        let mut removed = 0;
        for row in rows {
            let (id, pid, pid_start) = row?;
            if !pid_matches(pid, pid_start) {
                self.remove_attachment(id)?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn attachment_count(&self, environment_id: &str) -> anyhow::Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM attachments WHERE environment_id = ?1",
            params![environment_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub(super) fn live_attachment_count(&self, environment_id: &str) -> anyhow::Result<usize> {
        self.reap_dead_attachments(environment_id)?;
        self.attachment_count(environment_id)
    }

    pub(super) fn wait_until_no_attachments(&self, environment_id: &str) -> anyhow::Result<()> {
        let mut announced = false;
        loop {
            if self.live_attachment_count(environment_id)? == 0 {
                return Ok(());
            }
            if !announced {
                println!("Waiting for all attached shells to exit...");
                announced = true;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    pub(super) fn owner_is_gone(owner_pid: Option<u32>, owner_pid_start: Option<i64>) -> bool {
        match owner_pid {
            None => true,
            Some(pid) => !pid_matches(pid, owner_pid_start),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_invocation(
        &self,
        kind: InvocationKind,
        source_dir: &Path,
        package_name: &str,
        package_version: &str,
        distro: &DistroVersion,
        driver: Option<DriverType>,
        success: bool,
        changes_path: Option<&Path>,
        environment_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO invocations (
                kind, source_dir, package_name, package_version,
                distro_family, distro_codename, distro_version, distro_is_devel,
                driver, success, changes_path, environment_id, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                kind.as_str(),
                path_to_string(source_dir),
                package_name,
                package_version,
                distro.distro.as_str(),
                distro.codename,
                distro.version,
                distro.is_devel as i64,
                driver.map(DriverType::as_str),
                success as i64,
                changes_path.map(path_to_string),
                environment_id,
                created_at,
            ],
        )?;
        self.prune_invocations()?;
        Ok(())
    }

    /// Drop Invocation rows that can no longer affect a lookup.
    ///
    /// Keeps the newest row per kind, Source tree, and DistroVersion. A
    /// binary-build row is kept only when it succeeded and its `.changes`
    /// file still exists, so a later failure does not erase the last usable
    /// artifact and a deleted artifact does not linger.
    fn prune_invocations(&self) -> anyhow::Result<()> {
        let rows = {
            let mut stmt = self.conn.prepare(
                "SELECT id, kind, source_dir, distro_family, distro_codename,
                        success, changes_path
                 FROM invocations
                 ORDER BY created_at DESC, id DESC",
            )?;
            let mapped = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)? != 0,
                    row.get::<_, Option<String>>(6)?,
                ))
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };

        let mut seen = HashSet::new();
        let mut drop_ids = Vec::new();
        for (id, kind, source_dir, family, codename, success, changes_path) in &rows {
            let key = (
                kind.clone(),
                source_dir.clone(),
                family.clone(),
                codename.clone(),
            );
            let keep = if kind == "binary_build" {
                let file_exists = changes_path
                    .as_deref()
                    .is_some_and(|path| Path::new(path).is_file());
                *success && file_exists && seen.insert(key)
            } else {
                seen.insert(key)
            };
            if !keep {
                drop_ids.push(*id);
            }
        }
        if drop_ids.is_empty() {
            return Ok(());
        }

        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| -> anyhow::Result<()> {
            for id in drop_ids {
                self.conn
                    .execute("DELETE FROM invocations WHERE id = ?1", params![id])?;
            }
            Ok(())
        })();
        match &result {
            Ok(()) => self.conn.execute_batch("COMMIT")?,
            Err(_) => {
                let _ = self.conn.execute_batch("ROLLBACK");
            }
        }
        result
    }

    /// Successful binary-build Invocations for this Source tree whose `.changes` still exists,
    /// newest first. Missing files are pruned rather than retained.
    pub fn binary_build_changes_for_tree(
        &self,
        source_dir: &Path,
    ) -> anyhow::Result<Vec<RecordedInvocation>> {
        // Housekeeping only: the query below filters missing files anyway.
        if let Err(error) = self.prune_invocations() {
            eprintln!("Warning: failed to prune Invocation history: {error}");
        }
        let mut stmt = self.conn.prepare(
            "SELECT kind, source_dir, package_name, package_version,
                    distro_family, distro_codename, distro_version, distro_is_devel,
                    driver, success, changes_path
             FROM invocations
             WHERE source_dir = ?1 AND kind = 'binary_build' AND success = 1
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![path_to_string(source_dir)], |row| {
            Ok(RecordedInvocation {
                kind: InvocationKind::BinaryBuild,
                source_dir: PathBuf::from(row.get::<_, String>(1)?),
                package_version: row.get(3)?,
                distro: DistroVersion {
                    distro: Distro::parse(&row.get::<_, String>(4)?),
                    codename: row.get(5)?,
                    version: row.get(6)?,
                    is_devel: row.get::<_, i64>(7)? != 0,
                },
                driver: match row.get::<_, Option<String>>(8)? {
                    Some(name) => Some(DriverType::from_name(&name).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            8,
                            rusqlite::types::Type::Text,
                            anyhow!(error).into(),
                        )
                    })?),
                    None => None,
                },
                success: row.get::<_, i64>(9)? != 0,
                changes_path: row.get::<_, Option<String>>(10)?.map(PathBuf::from),
            })
        })?;

        let mut found = Vec::new();
        for row in rows {
            let invocation = row?;
            let Some(path) = invocation.changes_path.as_ref() else {
                continue;
            };
            if path.is_file() {
                found.push(invocation);
            }
        }
        Ok(found)
    }

    /// Every Invocation recorded for this Source tree, newest first.
    /// Includes failures. Does not prune.
    #[cfg(test)]
    pub(super) fn invocations_for_source(
        &self,
        source_dir: &Path,
    ) -> anyhow::Result<Vec<RecordedInvocation>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, source_dir, package_version,
                    distro_family, distro_codename, distro_version, distro_is_devel,
                    driver, success, changes_path
             FROM invocations
             WHERE source_dir = ?1
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![path_to_string(source_dir)], |row| {
            let kind = match row.get::<_, String>(0)?.as_str() {
                "binary_build" => InvocationKind::BinaryBuild,
                "source_build" => InvocationKind::SourceBuild,
                "test_run" => InvocationKind::TestRun,
                other => {
                    return Err(rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        anyhow!("unknown Invocation kind '{other}'").into(),
                    ));
                }
            };
            Ok(RecordedInvocation {
                kind,
                source_dir: PathBuf::from(row.get::<_, String>(1)?),
                package_version: row.get(2)?,
                distro: DistroVersion {
                    distro: Distro::parse(&row.get::<_, String>(3)?),
                    codename: row.get(4)?,
                    version: row.get(5)?,
                    is_devel: row.get::<_, i64>(6)? != 0,
                },
                driver: match row.get::<_, Option<String>>(7)? {
                    Some(name) => Some(DriverType::from_name(&name).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Text,
                            anyhow!(error).into(),
                        )
                    })?),
                    None => None,
                },
                success: row.get::<_, i64>(8)? != 0,
                changes_path: row.get::<_, Option<String>>(9)?.map(PathBuf::from),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

const ENVIRONMENT_COLUMNS: &str = "id, driver, package_name, package_identifier, source_dir,
     root_dir, distro_family, distro_codename, distro_version, distro_is_devel,
     persistent, purpose, owner_pid, owner_pid_start, driver_metadata";

fn row_to_registered(row: &rusqlite::Row<'_>) -> rusqlite::Result<RegisteredEnvironment> {
    let driver: String = row.get(1)?;
    let purpose: String = row.get(11)?;
    let metadata_json: String = row.get(14)?;
    let driver_metadata: HashMap<String, String> =
        serde_json::from_str(&metadata_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                14,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let owner_pid: Option<i64> = row.get(12)?;
    let owner_pid_start: Option<i64> = row.get(13)?;
    Ok(RegisteredEnvironment {
        environment: Environment {
            driver: DriverType::from_name(&driver).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    anyhow!(error).into(),
                )
            })?,
            package_name: row.get(2)?,
            package_identifier: row.get(3)?,
            source_dir: PathBuf::from(row.get::<_, String>(4)?),
            root_dir: PathBuf::from(row.get::<_, String>(5)?),
            distro: DistroVersion {
                distro: Distro::parse(&row.get::<_, String>(6)?),
                codename: row.get(7)?,
                version: row.get(8)?,
                is_devel: row.get::<_, i64>(9)? != 0,
            },
            persistence: Persistence::from_i64(row.get(10)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    10,
                    rusqlite::types::Type::Integer,
                    error.into(),
                )
            })?,
            purpose: EnvironmentPurpose::from_name(&purpose).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    11,
                    rusqlite::types::Type::Text,
                    anyhow!(error).into(),
                )
            })?,
        },
        driver_metadata,
        owner_pid: owner_pid.map(|pid| pid as u32),
        owner_pid_start,
    })
}

fn pid_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let result = unsafe { libc::kill(pid as i32, 0) };
    if result == 0 {
        return true;
    }
    std::io::Error::last_os_error()
        .raw_os_error()
        .is_none_or(|code| code != libc::ESRCH)
}

/// Start time of `pid` (field 22 of `/proc/<pid>/stat`), recorded alongside
/// every stored pid so PID reuse is detectable. `None` when unreadable
/// (non-Linux, or the process is already gone).
fn pid_start_time(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // comm (field 2) is parenthesized and may itself contain spaces or
    // parens; the remaining fields start after the last ')'.
    let after_comm = stat.rsplit_once(')')?.1;
    // starttime is field 22 overall, i.e. index 19 after comm.
    after_comm.split_whitespace().nth(19)?.parse().ok()
}

/// Whether `pid` is alive *and* still the same process instance we recorded
/// (matching start time). Falls back to plain liveness when the start time
/// cannot be verified (e.g. non-Linux host).
fn pid_matches(pid: u32, recorded_start: Option<i64>) -> bool {
    if !pid_is_alive(pid) {
        return false;
    }
    match (recorded_start, pid_start_time(pid)) {
        (Some(recorded), Some(current)) => recorded as u64 == current,
        _ => true,
    }
}

pub(super) fn current_pid() -> u32 {
    std::process::id()
}

/// 0 / 1 / 2+ selection for `debmagic env shell` with no explicit id.
///
/// Environments of the Source tree's current `package_identifier` win over
/// those left behind by earlier versions (each version is its own
/// Environment), so a changelog bump does not make the choice ambiguous.
pub fn select_unique_environment<'a>(
    environments: &'a [RegisteredEnvironment],
    current_package_identifier: &str,
) -> anyhow::Result<&'a RegisteredEnvironment> {
    let current: Vec<&RegisteredEnvironment> = environments
        .iter()
        .filter(|registered| {
            registered.environment.package_identifier == current_package_identifier
        })
        .collect();
    let candidates: Vec<&RegisteredEnvironment> = if current.is_empty() {
        environments.iter().collect()
    } else {
        current
    };
    match candidates.as_slice() {
        [] => bail!("no Environment found for this Source tree"),
        [only] => Ok(*only),
        many => {
            let mut message = String::from(
                "more than one Environment exists for this Source tree; pass an Environment id:\n",
            );
            for registered in many {
                message.push_str(&format!(
                    "  {}  {}  {}  {}  {}\n",
                    registered.environment.id(),
                    registered.environment.purpose.as_str(),
                    registered.environment.driver.as_str(),
                    registered.environment.distro.codename,
                    registered.environment.package_identifier,
                ));
            }
            bail!("{}", message.trim_end());
        }
    }
}

/// Latest Invocation per DistroVersion. `invocations` must be newest-first.
fn matching_binary_changes<'a>(
    invocations: &'a [RecordedInvocation],
    distro_filter: Option<&str>,
) -> Vec<&'a RecordedInvocation> {
    let mut matching: Vec<&RecordedInvocation> = invocations
        .iter()
        .filter(|invocation| {
            distro_filter.is_none_or(|codename| invocation.distro.codename == codename)
        })
        .collect();

    let mut seen = HashMap::new();
    matching.retain(|invocation| {
        seen.insert(
            (
                invocation.distro.distro.as_str().to_string(),
                invocation.distro.codename.clone(),
            ),
            (),
        )
        .is_none()
    });
    matching
}

/// 0 / 1 / 2+ DistroVersions among binary-build Invocations with existing `.changes`.
pub(super) fn select_unique_binary_changes(
    invocations: &[RecordedInvocation],
    distro_filter: Option<&str>,
) -> anyhow::Result<RecordedInvocation> {
    match matching_binary_changes(invocations, distro_filter).as_slice() {
        [] => {
            if let Some(codename) = distro_filter {
                bail!(
                    "no binary-build Invocation with a .changes file for distro {codename}; run `debmagic build binary` first"
                );
            }
            bail!(
                "no prior binary build found; run `debmagic build binary` first or pass --changes"
            )
        }
        [only] => Ok((*only).clone()),
        many => {
            let mut message = String::from(
                "more than one DistroVersion has a binary-build Invocation; pass --distro:\n",
            );
            for invocation in many {
                message.push_str(&format!("  {}\n", invocation.distro.codename));
            }
            bail!("{}", message.trim_end());
        }
    }
}

/// Prior binary-build Invocation used to default driver and distro.
///
/// `--changes` overrides only the artifact path, so history is still consulted.
/// An explicit `.changes` with no matching Invocation is not an error (the
/// caller then requires `--driver` and `--distro`). Several DistroVersions
/// still are.
pub fn prior_binary_invocation(
    invocations: &[RecordedInvocation],
    distro_filter: Option<&str>,
    changes_explicit: bool,
) -> anyhow::Result<Option<RecordedInvocation>> {
    let matching = matching_binary_changes(invocations, distro_filter);
    if changes_explicit && matching.is_empty() {
        return Ok(None);
    }
    select_unique_binary_changes(invocations, distro_filter).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::DriverType;
    use rusqlite::Connection;

    fn temp_registry() -> (PathBuf, Registry) {
        let dir = std::env::temp_dir().join(format!("debmagic-registry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let registry = Registry::open(&dir.join("db.sqlite")).unwrap();
        (dir, registry)
    }

    fn sample_environment(source_dir: &Path, purpose: EnvironmentPurpose) -> Environment {
        Environment::new(
            DriverType::Docker,
            "pkg",
            "pkg-1.0",
            source_dir,
            DistroVersion::new(Distro::Debian, "trixie", "13"),
            Persistence::Always,
            purpose,
            &source_dir.join("envs"),
        )
    }

    #[test]
    fn environment_id_differs_across_source_trees_and_purposes() {
        let first = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        let second = sample_environment(Path::new("/src/b"), EnvironmentPurpose::Build);
        let test = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Test);
        assert_ne!(first.id(), second.id());
        assert_ne!(first.id(), test.id());
        assert_eq!(
            first.id(),
            sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build).id()
        );
        assert_eq!(first.id().len(), 16);
    }

    #[test]
    fn upsert_and_list_round_trip() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry();
        let _ = &dir;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        registry.upsert_environment(&environment, Some(1))?;
        let listed = registry.list()?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].environment.id(), environment.id());
        assert_eq!(listed[0].environment.source_dir, PathBuf::from("/src/a"));
        assert_eq!(listed[0].owner_pid, Some(1));
        Ok(())
    }

    #[test]
    fn unique_shell_selection_is_zero_one_many() {
        let build = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        let test = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Test);
        let registered = |environment| RegisteredEnvironment {
            environment,
            driver_metadata: HashMap::new(),
            owner_pid: None,
            owner_pid_start: None,
        };
        let one = vec![registered(build.clone())];
        assert!(select_unique_environment(&one, "pkg-1.0").is_ok());
        assert!(select_unique_environment(&[], "pkg-1.0").is_err());
        let two = vec![registered(build), registered(test)];
        let error = select_unique_environment(&two, "pkg-1.0")
            .unwrap_err()
            .to_string();
        assert!(error.contains("more than one Environment"));
    }

    #[test]
    fn shell_selection_prefers_the_current_package_version() {
        let registered = |identifier: &str| RegisteredEnvironment {
            environment: Environment::new(
                DriverType::Docker,
                "pkg",
                identifier,
                Path::new("/src/a"),
                DistroVersion::new(Distro::Debian, "trixie", "13"),
                Persistence::Always,
                EnvironmentPurpose::Build,
                Path::new("/envs"),
            ),
            driver_metadata: HashMap::new(),
            owner_pid: None,
            owner_pid_start: None,
        };
        let both = vec![registered("pkg-1.0"), registered("pkg-1.1")];
        let picked = select_unique_environment(&both, "pkg-1.1").unwrap();
        assert_eq!(picked.environment.package_identifier, "pkg-1.1");

        // No Environment of the current version: fall back to all of them.
        let error = select_unique_environment(&both, "pkg-2.0")
            .unwrap_err()
            .to_string();
        assert!(error.contains("more than one Environment"));
    }

    #[test]
    fn begin_environment_refuses_live_owner_and_allows_takeover() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry();
        let _ = &dir;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);

        let none = HashMap::new();

        // pid 1 is always alive; a different process must not take over.
        registry.begin_environment(&environment, 1, &none)?;
        let error = registry
            .begin_environment(&environment, current_pid(), &none)
            .unwrap_err()
            .to_string();
        assert!(error.contains("already in use by process 1"));

        // A dead owner can be taken over.
        let mut child = std::process::Command::new("true").spawn()?;
        let dead_pid = child.id();
        child.wait()?;
        registry.upsert_environment(&environment, Some(dead_pid))?;
        registry.begin_environment(&environment, current_pid(), &none)?;

        // The same process re-beginning is idempotent.
        registry.begin_environment(&environment, current_pid(), &none)?;
        Ok(())
    }

    #[test]
    fn begin_environment_refuses_live_attachments() -> anyhow::Result<()> {
        let (_dir, registry) = temp_registry();
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        registry.upsert_environment(&environment, None)?;
        registry.add_attachment(&environment.id(), current_pid())?;

        let error = registry
            .begin_environment(&environment, current_pid(), &HashMap::new())
            .unwrap_err()
            .to_string();
        assert!(error.contains("live Attachment"), "{error}");
        Ok(())
    }

    #[test]
    fn begin_environment_records_planned_metadata_and_returns_previous() -> anyhow::Result<()> {
        let (_dir, registry) = temp_registry();
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        let first = HashMap::from([("project".to_string(), "old".to_string())]);
        let second = HashMap::from([("project".to_string(), "new".to_string())]);

        assert!(
            registry
                .begin_environment(&environment, current_pid(), &first)?
                .is_none()
        );
        let previous = registry
            .begin_environment(&environment, current_pid(), &second)?
            .expect("previous row");
        assert_eq!(previous.driver_metadata, first);
        assert_eq!(
            registry.get(&environment.id())?.unwrap().driver_metadata,
            second
        );
        Ok(())
    }

    #[test]
    fn released_destroy_claim_allows_attachments_again() -> anyhow::Result<()> {
        let (_dir, registry) = temp_registry();
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        registry.upsert_environment(&environment, None)?;
        registry.claim_for_destroy(&environment.id())?;
        registry.release_destroy_claim(&environment.id(), current_pid())?;

        registry.add_attachment(&environment.id(), current_pid())?;
        assert_eq!(registry.get(&environment.id())?.unwrap().owner_pid, None);
        Ok(())
    }

    #[test]
    fn claim_for_destroy_blocks_attachments_and_finish_removes_row() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry();
        let _ = &dir;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        registry.upsert_environment(&environment, None)?;

        registry.claim_for_destroy(&environment.id())?;
        let error = registry
            .add_attachment(&environment.id(), current_pid())
            .unwrap_err()
            .to_string();
        assert!(error.contains("gone or being destroyed"));

        registry.finish_destroy(&environment.id(), current_pid())?;
        assert!(registry.get(&environment.id())?.is_none());
        Ok(())
    }

    #[test]
    fn claim_for_destroy_refuses_live_attachments() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry();
        let _ = &dir;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        registry.upsert_environment(&environment, None)?;
        registry.add_attachment(&environment.id(), current_pid())?;

        let error = registry
            .claim_for_destroy(&environment.id())
            .unwrap_err()
            .to_string();
        assert!(error.contains("live Attachments"));
        Ok(())
    }

    #[test]
    fn invocation_records_full_distro_version() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry();
        let source = dir.join("src");
        std::fs::create_dir_all(&source)?;
        let changes = dir.join("pkg.changes");
        std::fs::write(&changes, "changes")?;

        let distro = DistroVersion {
            distro: Distro::Custom("yocto".to_string()),
            codename: "kirkstone".to_string(),
            version: "4.0".to_string(),
            is_devel: true,
        };
        registry.record_invocation(
            InvocationKind::BinaryBuild,
            &source,
            "pkg",
            "1.0",
            &distro,
            Some(DriverType::Docker),
            true,
            Some(&changes),
            None,
        )?;

        let found = registry.binary_build_changes_for_tree(&source)?;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].distro, distro);
        Ok(())
    }

    #[test]
    fn binary_changes_lookup_skips_missing_files_and_splits_on_distro() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry();
        let source = dir.join("src");
        std::fs::create_dir_all(&source)?;
        let trixie = dir.join("trixie.changes");
        std::fs::write(&trixie, "changes")?;
        let missing = dir.join("gone.changes");

        let distro_trixie = DistroVersion::new(Distro::Debian, "trixie", "13");
        let distro_sid = DistroVersion::new(Distro::Debian, "sid", "");
        registry.record_invocation(
            InvocationKind::BinaryBuild,
            &source,
            "pkg",
            "1.0",
            &distro_trixie,
            Some(DriverType::Docker),
            true,
            Some(&missing),
            None,
        )?;
        registry.record_invocation(
            InvocationKind::BinaryBuild,
            &source,
            "pkg",
            "1.0",
            &distro_trixie,
            Some(DriverType::Docker),
            true,
            Some(&trixie),
            None,
        )?;
        registry.record_invocation(
            InvocationKind::BinaryBuild,
            &source,
            "pkg",
            "1.0",
            &distro_sid,
            Some(DriverType::Docker),
            true,
            Some(&trixie),
            None,
        )?;

        assert_eq!(invocation_count(&registry)?, 2);

        let found = registry.binary_build_changes_for_tree(&source)?;
        let error = select_unique_binary_changes(&found, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("more than one DistroVersion"));
        let picked = select_unique_binary_changes(&found, Some("trixie"))?;
        assert_eq!(picked.changes_path.as_deref(), Some(trixie.as_path()));
        Ok(())
    }

    fn invocation_count(registry: &Registry) -> anyhow::Result<i64> {
        let count = registry
            .conn
            .query_row("SELECT COUNT(*) FROM invocations", [], |row| row.get(0))?;
        Ok(count)
    }

    #[test]
    fn invocation_history_keeps_the_newest_usable_binary_build() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry();
        let source = dir.join("src");
        std::fs::create_dir_all(&source)?;
        let older = dir.join("older.changes");
        let newer = dir.join("newer.changes");
        std::fs::write(&older, "old")?;
        std::fs::write(&newer, "new")?;
        let distro = DistroVersion::new(Distro::Debian, "trixie", "13");

        registry.record_invocation(
            InvocationKind::BinaryBuild,
            &source,
            "pkg",
            "1.0",
            &distro,
            Some(DriverType::Docker),
            true,
            Some(&older),
            None,
        )?;
        registry.record_invocation(
            InvocationKind::BinaryBuild,
            &source,
            "pkg",
            "1.1",
            &distro,
            Some(DriverType::Lxd),
            true,
            Some(&newer),
            None,
        )?;
        registry.record_invocation(
            InvocationKind::BinaryBuild,
            &source,
            "pkg",
            "1.2",
            &distro,
            Some(DriverType::Docker),
            false,
            None,
            None,
        )?;

        assert_eq!(invocation_count(&registry)?, 1);
        let found = registry.binary_build_changes_for_tree(&source)?;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].changes_path.as_deref(), Some(newer.as_path()));
        assert_eq!(found[0].driver, Some(DriverType::Lxd));
        assert_eq!(found[0].package_version, "1.1");
        Ok(())
    }

    #[test]
    fn deleting_a_changes_file_prunes_that_invocation() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry();
        let source = dir.join("src");
        std::fs::create_dir_all(&source)?;
        let changes = dir.join("pkg.changes");
        std::fs::write(&changes, "changes")?;
        let distro = DistroVersion::new(Distro::Debian, "trixie", "13");
        registry.record_invocation(
            InvocationKind::BinaryBuild,
            &source,
            "pkg",
            "1.0",
            &distro,
            Some(DriverType::Docker),
            true,
            Some(&changes),
            None,
        )?;
        std::fs::remove_file(&changes)?;

        let found = registry.binary_build_changes_for_tree(&source)?;
        assert!(found.is_empty());
        assert_eq!(invocation_count(&registry)?, 0);
        Ok(())
    }

    #[test]
    fn non_binary_invocations_collapse_to_the_newest() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry();
        let source = dir.join("src");
        let distro = DistroVersion::new(Distro::Debian, "trixie", "13");
        for version in ["1.0", "1.1"] {
            registry.record_invocation(
                InvocationKind::TestRun,
                &source,
                "pkg",
                version,
                &distro,
                Some(DriverType::Docker),
                true,
                None,
                None,
            )?;
        }
        assert_eq!(invocation_count(&registry)?, 1);
        Ok(())
    }

    #[test]
    fn open_if_exists_does_not_create_the_registry() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!(
            "debmagic-missing-{}/db.sqlite",
            uuid::Uuid::new_v4()
        ));
        let opened = Registry::open_if_exists(&path)?;
        assert!(opened.is_none());
        assert!(!path.exists());
        assert!(!path.parent().unwrap().exists());
        Ok(())
    }

    #[test]
    fn explicit_changes_still_reads_a_prior_invocation() {
        let trixie = recorded_binary("trixie", "/out/trixie.changes");
        let sid = recorded_binary("sid", "/out/sid.changes");

        let prior = prior_binary_invocation(std::slice::from_ref(&trixie), None, true)
            .unwrap()
            .unwrap();
        assert_eq!(prior.driver, Some(DriverType::Docker));
        assert_eq!(prior.distro.codename, "trixie");

        assert!(prior_binary_invocation(&[], None, true).unwrap().is_none());

        let error = prior_binary_invocation(&[trixie, sid], None, true)
            .unwrap_err()
            .to_string();
        assert!(error.contains("more than one DistroVersion"));

        let missing = prior_binary_invocation(&[], None, false).unwrap_err();
        assert!(missing.to_string().contains("no prior binary build"));
    }

    fn recorded_binary(codename: &str, changes: &str) -> RecordedInvocation {
        RecordedInvocation {
            kind: InvocationKind::BinaryBuild,
            source_dir: PathBuf::from("/src"),
            package_version: "1.0".to_string(),
            distro: DistroVersion::new(Distro::Debian, codename, "13"),
            driver: Some(DriverType::Docker),
            success: true,
            changes_path: Some(PathBuf::from(changes)),
        }
    }

    fn pragma_user_version(path: &Path) -> i64 {
        let conn = Connection::open(path).unwrap();
        conn.pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn opening_a_new_registry_stamps_schema_version_2() -> anyhow::Result<()> {
        let (dir, _registry) = temp_registry();
        assert_eq!(pragma_user_version(&dir.join("db.sqlite")), 2);
        Ok(())
    }

    #[test]
    fn reopening_a_registry_does_not_rebuild_the_schema() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry();
        let path = dir.join("db.sqlite");
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        registry.upsert_environment(&environment, None)?;
        drop(registry);

        let registry = Registry::open(&path)?;
        let listed = registry.list()?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].environment.id(), environment.id());
        drop(registry);
        assert_eq!(pragma_user_version(&path), 2);
        Ok(())
    }

    #[test]
    fn version_1_boolean_reads_as_no_and_always() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("debmagic-registry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("db.sqlite");
        let conn = Connection::open(&path)?;
        conn.execute_batch(include_str!("../migrations/0001.sql"))?;
        conn.execute(
            "INSERT INTO environments (
                id, driver, package_name, package_identifier, source_dir, root_dir,
                distro_family, distro_codename, distro_version, distro_is_devel,
                persistent, purpose, driver_metadata
            ) VALUES
                ('kept', 'docker', 'pkg', 'pkg-1.0', '/src', '/root/kept',
                 'debian', 'trixie', '13', 0, 1, 'build', '{}'),
                ('gone', 'docker', 'pkg', 'pkg-1.0', '/src', '/root/gone',
                 'debian', 'trixie', '13', 0, 0, 'test', '{}')",
            [],
        )?;
        conn.pragma_update(None, "user_version", 1i64)?;
        drop(conn);

        let registry = Registry::open(&path)?;
        let listed = registry.list()?;
        let gone = listed
            .iter()
            .find(|row| row.environment.purpose == EnvironmentPurpose::Test)
            .unwrap();
        let kept = listed
            .iter()
            .find(|row| row.environment.purpose == EnvironmentPurpose::Build)
            .unwrap();
        assert_eq!(gone.environment.persistence, Persistence::No);
        assert_eq!(kept.environment.persistence, Persistence::Always);
        assert_eq!(pragma_user_version(&path), 2);
        Ok(())
    }

    #[test]
    fn newer_schema_is_refused() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("debmagic-registry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("db.sqlite");
        let conn = Connection::open(&path)?;
        conn.pragma_update(None, "user_version", 99i64)?;
        drop(conn);

        let Err(error) = Registry::open(&path) else {
            panic!("expected a newer schema to be refused");
        };
        let rendered = format!("{error:#}");
        assert!(rendered.contains("upgrade debmagic"), "{rendered}");
        assert!(rendered.contains("99"), "{rendered}");
        Ok(())
    }

    #[test]
    fn open_or_ephemeral_does_not_hide_a_newer_schema() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("debmagic-registry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("db.sqlite");
        let conn = Connection::open(&path)?;
        conn.pragma_update(None, "user_version", 99i64)?;
        drop(conn);

        let Err(error) = Registry::open_or_ephemeral(&path) else {
            panic!("expected a newer schema to be refused");
        };
        let rendered = format!("{error:#}");
        assert!(rendered.contains("upgrade debmagic"), "{rendered}");
        Ok(())
    }

    #[test]
    fn open_or_ephemeral_falls_back_when_the_file_is_unusable() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("debmagic-registry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir)?;
        let registry = Registry::open_or_ephemeral(&dir)?;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        registry.upsert_environment(&environment, None)?;
        assert_eq!(registry.list()?.len(), 1);
        Ok(())
    }

    #[test]
    fn busy_registry_is_not_mistaken_for_an_unusable_one() {
        let busy = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            None,
        );
        assert!(is_busy(&anyhow::Error::new(busy).context("opening")));
        assert!(!is_busy(&anyhow!("not a directory")));
    }

    #[test]
    fn concurrent_first_opens_all_succeed() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("debmagic-registry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("db.sqlite");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    Registry::open(&path).map(|_| ())
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("opener thread panicked")?;
        }
        assert_eq!(pragma_user_version(&path), 2);
        Ok(())
    }

    #[test]
    fn failed_migration_does_not_stamp_the_schema_version() -> anyhow::Result<()> {
        let mut conn = Connection::open_in_memory()?;
        let steps: &[&str] = &["CREATE TABLE items (id INTEGER PRIMARY KEY); NOT SQL;"];

        let error = migrate_with(&mut conn, steps).unwrap_err();
        let rendered = format!("{error:#}");
        assert!(rendered.contains("syntax error"), "{rendered}");
        assert_eq!(user_version(&conn)?, 0);

        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'items'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(exists, 0);
        Ok(())
    }
}
