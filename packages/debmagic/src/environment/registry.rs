//! Storage of the Environment registry: rows of Environments, their
//! Attachments, and Invocations. Row functions take a connection, so the
//! parent module composes them into transactions around its lifecycle rules.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail};
use sqlx::{
    migrate::{MigrateError, Migrator},
    pool::PoolConnection,
    sqlite::{Sqlite, SqliteConnectOptions, SqliteConnection, SqlitePool, SqlitePoolOptions},
};

use super::process::Process;
use crate::build_intent::BuildKind;
use crate::driver::{
    DriverType, Environment, EnvironmentMetadata, EnvironmentPurpose, Persistence,
};
use debmagic_common::distro::{Distro, DistroVersion};

macro_rules! select_environments {
    ($rest:literal) => {
        concat!(
            "select driver, package_name, package_identifier, source_dir, root_dir,
                distro_family, distro_codename, distro_version, distro_is_devel,
                persistence, purpose, owner_pid, owner_pid_start, destroying, driver_metadata
            from environments ",
            $rest
        )
    };
}

macro_rules! select_invocations {
    ($rest:literal) => {
        concat!(
            "select id, kind, source_dir, package_name, package_version,
                distro_family, distro_codename, distro_version, distro_is_devel,
                driver, success, changes_path, environment_id
            from invocations ",
            $rest
        )
    };
}

static MIGRATOR: Migrator = sqlx::migrate!("src/migrations");

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub type Transaction = sqlx::Transaction<'static, Sqlite>;

#[derive(Debug)]
struct IncompatibleSchema {
    version: i64,
}

impl std::fmt::Display for IncompatibleSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the Environment registry has schema version {}, which this debmagic does not know; upgrade debmagic",
            self.version
        )
    }
}

impl std::error::Error for IncompatibleSchema {}

fn is_incompatible_schema(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.downcast_ref::<IncompatibleSchema>().is_some())
}

fn is_busy(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<sqlx::Error>()
            .is_some_and(is_busy_error)
    })
}

/// SQLITE_BUSY or SQLITE_LOCKED; extended result codes add the high bits.
fn is_busy_error(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(error) = error else {
        return false;
    };
    error
        .code()
        .and_then(|code| code.parse::<i32>().ok())
        .is_some_and(|code| matches!(code & 0xff, 5 | 6))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationKind {
    Build(BuildKind),
    Test,
}

impl InvocationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Build(BuildKind::Binary) => "binary_build",
            Self::Build(BuildKind::Source) => "source_build",
            Self::Test => "test_run",
        }
    }

    fn from_name(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "binary_build" => Self::Build(BuildKind::Binary),
            "source_build" => Self::Build(BuildKind::Source),
            "test_run" => Self::Test,
            other => bail!("unknown Invocation kind '{other}'"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct RecordedInvocation {
    pub kind: InvocationKind,
    pub source_dir: PathBuf,
    pub package_name: String,
    pub package_version: String,
    pub distro: DistroVersion,
    pub driver: Option<DriverType>,
    pub success: bool,
    pub changes_path: Option<PathBuf>,
    pub environment_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegisteredEnvironment {
    pub environment: Environment,
    pub(super) driver_metadata: HashMap<String, String>,
    /// The command holding the Environment.
    pub(super) owner: Option<Process>,
    pub(super) destroying: bool,
}

impl RegisteredEnvironment {
    pub(super) fn metadata(&self) -> EnvironmentMetadata {
        EnvironmentMetadata {
            environment: self.environment.clone(),
            driver_metadata: self.driver_metadata.clone(),
        }
    }
}

pub(super) struct Attachment {
    pub id: i64,
    pub process: Process,
}

pub struct Registry {
    pool: SqlitePool,
}

impl Registry {
    async fn connect(options: SqliteConnectOptions) -> anyhow::Result<Self> {
        // One connection: no caller holds two at once, and SQLite serializes
        // writers anyway. Never closed while idle, so an in-memory registry
        // lives as long as the process.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options.busy_timeout(BUSY_TIMEOUT).foreign_keys(true))
            .await?;
        Ok(Self { pool })
    }

    pub async fn open(path: &Path) -> anyhow::Result<Self> {
        Self::open_file(path)
            .await
            .with_context(|| format!("opening the Environment registry {} failed", path.display()))
    }

    async fn open_file(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {} failed", parent.display()))?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let registry = Self::connect(options).await?;
        registry.enable_wal().await?;
        registry.migrate().await?;
        Ok(registry)
    }

    /// Open `path` when the file already exists. Does not create the file or
    /// its parent directory.
    pub async fn open_if_exists(path: &Path) -> anyhow::Result<Option<Self>> {
        if !path.is_file() {
            return Ok(None);
        }
        Self::open(path).await.map(Some)
    }

    /// Open the on-disk registry when it already exists. `debmagic env` reads
    /// use this so listing or cleaning does not create an empty database.
    pub async fn open_default_if_exists() -> anyhow::Result<Option<Self>> {
        Self::open_if_exists(&crate::data_dir::registry_path()?).await
    }

    async fn open_memory() -> anyhow::Result<Self> {
        let registry = Self::connect(SqliteConnectOptions::new().in_memory(true)).await?;
        registry.migrate().await?;
        Ok(registry)
    }

    async fn warn_and_open_memory(error: &anyhow::Error) -> Self {
        eprintln!(
            "Warning: cannot open the Environment registry ({error:#}); \
             continuing without persistent bookkeeping"
        );
        Self::open_memory()
            .await
            .expect("an in-memory registry cannot fail to open")
    }

    /// Open `path`, or a process-local in-memory registry when the file cannot
    /// be used. Still an error:
    /// - a schema newer than this binary — falling back would hide
    ///   Environments the newer debmagic already recorded;
    /// - a registry that is busy or locked — it works, another debmagic holds
    ///   it, and bypassing it would skip the "already in use" check that keeps
    ///   two runs off the same Environment.
    async fn open_or_ephemeral(path: &Path) -> anyhow::Result<Self> {
        match Self::open(path).await {
            Ok(registry) => Ok(registry),
            Err(error) if is_incompatible_schema(&error) || is_busy(&error) => Err(error),
            Err(error) => Ok(Self::warn_and_open_memory(&error).await),
        }
    }

    /// For build/test bookkeeping, which must not fail the actual work: see
    /// [`Self::open_or_ephemeral`]. `debmagic env` commands *are* registry
    /// operations and use [`Self::open_default_if_exists`].
    pub async fn open_default_or_ephemeral() -> anyhow::Result<Self> {
        match crate::data_dir::registry_path() {
            Ok(path) => Self::open_or_ephemeral(&path).await,
            Err(error) => Ok(Self::warn_and_open_memory(&error).await),
        }
    }

    /// Switch to WAL, retrying on SQLITE_BUSY. Changing the journal mode
    /// upgrades a shared lock, and SQLite reports that conflict immediately
    /// instead of invoking the busy handler, so concurrent first opens must
    /// retry by hand.
    async fn enable_wal(&self) -> anyhow::Result<()> {
        let deadline = Instant::now() + BUSY_TIMEOUT;
        loop {
            match sqlx::query("pragma journal_mode = wal")
                .execute(&self.pool)
                .await
            {
                Ok(_) => return Ok(()),
                Err(error) if is_busy_error(&error) && Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => return Err(error).context("enabling WAL failed"),
            }
        }
    }

    async fn migrate(&self) -> anyhow::Result<()> {
        // The SQLite migrator does not lock, so concurrent first opens are
        // serialized by running it inside our own write transaction.
        let mut tx = self.write().await?;
        match MIGRATOR.run(&mut *tx).await {
            Ok(()) => {}
            Err(MigrateError::VersionMissing(version)) => {
                return Err(IncompatibleSchema { version }.into());
            }
            Err(MigrateError::VersionMismatch(version)) => bail!(
                "schema version {version} of the Environment registry differs from this debmagic's; \
                 delete the registry to recreate it"
            ),
            Err(error) => return Err(error).context("migrating the schema failed"),
        }
        let violations = sqlx::query("pragma foreign_key_check")
            .fetch_all(&mut *tx)
            .await
            .context("checking foreign keys failed")?;
        if !violations.is_empty() {
            bail!(
                "the Environment registry has {} foreign key violations",
                violations.len()
            );
        }
        tx.commit().await?;
        Ok(())
    }

    /// A write transaction. IMMEDIATE takes the write lock up front, so a
    /// read-check-write sequence cannot interleave with another debmagic.
    pub(super) async fn write(&self) -> anyhow::Result<Transaction> {
        self.pool
            .begin_with("begin immediate")
            .await
            .context("starting an Environment registry transaction failed")
    }

    /// The connection for a single statement outside a transaction. Release
    /// it before calling another registry function: there is only one.
    pub(super) async fn connection(&self) -> anyhow::Result<PoolConnection<Sqlite>> {
        self.pool
            .acquire()
            .await
            .context("connecting to the Environment registry failed")
    }

    pub async fn get(&self, id: &str) -> anyhow::Result<Option<RegisteredEnvironment>> {
        get_environment(&mut *self.connection().await?, id).await
    }

    pub async fn list(&self) -> anyhow::Result<Vec<RegisteredEnvironment>> {
        let rows = sqlx::query_as::<_, EnvironmentRow>(select_environments!(
            "order by package_name, source_dir, purpose, driver"
        ))
        .fetch_all(&mut *self.connection().await?)
        .await
        .context("listing the Environments failed")?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    pub async fn list_for_source_tree(
        &self,
        source_dir: &Path,
    ) -> anyhow::Result<Vec<RegisteredEnvironment>> {
        let rows = sqlx::query_as::<_, EnvironmentRow>(select_environments!(
            "where source_dir = ? order by purpose, driver, distro_codename"
        ))
        .bind(path_to_string(source_dir))
        .fetch_all(&mut *self.connection().await?)
        .await
        .context("listing the Environments of the Source tree failed")?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    pub async fn record_invocation(&self, invocation: &RecordedInvocation) -> anyhow::Result<()> {
        let mut tx = self.write().await?;
        sqlx::query(
            "insert into invocations (
                kind, source_dir, package_name, package_version,
                distro_family, distro_codename, distro_version, distro_is_devel,
                driver, success, changes_path, environment_id, created_at
            ) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(invocation.kind.as_str())
        .bind(path_to_string(&invocation.source_dir))
        .bind(&invocation.package_name)
        .bind(&invocation.package_version)
        .bind(invocation.distro.distro.as_str())
        .bind(&invocation.distro.codename)
        .bind(&invocation.distro.version)
        .bind(invocation.distro.is_devel)
        .bind(invocation.driver.map(DriverType::as_str))
        .bind(invocation.success)
        .bind(invocation.changes_path.as_deref().map(path_to_string))
        .bind(&invocation.environment_id)
        .bind(unix_time(SystemTime::now()))
        .execute(&mut *tx)
        .await
        .context("inserting the Invocation failed")?;
        prune_invocations(&mut tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Successful binary-build Invocations for this Source tree whose
    /// `.changes` still exists, newest first.
    pub async fn binary_build_changes_for_tree(
        &self,
        source_dir: &Path,
    ) -> anyhow::Result<Vec<RecordedInvocation>> {
        // Housekeeping only: the filter below skips missing files anyway.
        if let Err(error) = self.prune_invocations().await {
            eprintln!("Warning: failed to prune Invocation history: {error:#}");
        }
        let rows = sqlx::query_as::<_, InvocationRow>(select_invocations!(
            "where source_dir = ? and kind = ? and success = 1 order by created_at desc, id desc"
        ))
        .bind(path_to_string(source_dir))
        .bind(InvocationKind::Build(BuildKind::Binary).as_str())
        .fetch_all(&mut *self.connection().await?)
        .await
        .context("reading binary-build Invocations failed")?;
        let mut found = Vec::new();
        for row in rows {
            let invocation = RecordedInvocation::try_from(row)?;
            if invocation
                .changes_path
                .as_ref()
                .is_some_and(|path| path.is_file())
            {
                found.push(invocation);
            }
        }
        Ok(found)
    }

    async fn prune_invocations(&self) -> anyhow::Result<()> {
        let mut tx = self.write().await?;
        prune_invocations(&mut tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Every Invocation recorded for this Source tree, newest first.
    #[cfg(test)]
    pub(super) async fn invocations_for_source(
        &self,
        source_dir: &Path,
    ) -> anyhow::Result<Vec<RecordedInvocation>> {
        let rows = sqlx::query_as::<_, InvocationRow>(select_invocations!(
            "where source_dir = ? order by created_at desc, id desc"
        ))
        .bind(path_to_string(source_dir))
        .fetch_all(&mut *self.connection().await?)
        .await?;
        rows.into_iter().map(TryInto::try_into).collect()
    }
}

/// Drop Invocation rows that can no longer affect a lookup.
///
/// Keeps the newest row per kind, Source tree, and DistroVersion. A
/// binary-build row is kept only when it succeeded and its `.changes`
/// file still exists, so a later failure does not erase the last usable
/// artifact and a deleted artifact does not linger.
async fn prune_invocations(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    let rows = sqlx::query_as::<_, InvocationRow>(select_invocations!(
        "order by created_at desc, id desc"
    ))
    .fetch_all(&mut *conn)
    .await
    .context("reading the Invocation history failed")?;
    let binary_build = InvocationKind::Build(BuildKind::Binary).as_str();
    let mut seen = HashSet::new();
    for row in rows {
        let usable = row.kind != binary_build
            || (row.success
                && row
                    .changes_path
                    .as_deref()
                    .is_some_and(|path| Path::new(path).is_file()));
        let key = (
            row.kind,
            row.source_dir,
            row.distro_family,
            row.distro_codename,
        );
        if !(usable && seen.insert(key)) {
            sqlx::query("delete from invocations where id = ?")
                .bind(row.id)
                .execute(&mut *conn)
                .await
                .context("pruning the Invocation history failed")?;
        }
    }
    Ok(())
}

pub(super) async fn get_environment(
    conn: &mut SqliteConnection,
    id: &str,
) -> anyhow::Result<Option<RegisteredEnvironment>> {
    sqlx::query_as::<_, EnvironmentRow>(select_environments!("where id = ?"))
        .bind(id)
        .fetch_optional(conn)
        .await
        .context("reading the Environment from the registry failed")?
        .map(TryInto::try_into)
        .transpose()
}

/// Insert or replace `environment`, keeping its Driver metadata, apt
/// upgrade time, Attachments, and Invocations.
pub(super) async fn upsert_environment(
    conn: &mut SqliteConnection,
    environment: &Environment,
    owner: Option<Process>,
) -> anyhow::Result<()> {
    sqlx::query(
        "insert into environments (
            id, driver, package_name, package_identifier, source_dir, root_dir,
            distro_family, distro_codename, distro_version, distro_is_devel,
            persistence, purpose, owner_pid, owner_pid_start
        ) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        on conflict(id) do update set
            driver = excluded.driver,
            package_name = excluded.package_name,
            package_identifier = excluded.package_identifier,
            source_dir = excluded.source_dir,
            root_dir = excluded.root_dir,
            distro_family = excluded.distro_family,
            distro_codename = excluded.distro_codename,
            distro_version = excluded.distro_version,
            distro_is_devel = excluded.distro_is_devel,
            persistence = excluded.persistence,
            purpose = excluded.purpose,
            owner_pid = excluded.owner_pid,
            owner_pid_start = excluded.owner_pid_start,
            destroying = 0",
    )
    .bind(environment.id())
    .bind(environment.driver.as_str())
    .bind(&environment.package_name)
    .bind(&environment.package_identifier)
    .bind(path_to_string(&environment.source_dir))
    .bind(path_to_string(&environment.root_dir))
    .bind(environment.distro.distro.as_str())
    .bind(&environment.distro.codename)
    .bind(&environment.distro.version)
    .bind(environment.distro.is_devel)
    .bind(environment.persistence.as_str())
    .bind(environment.purpose.as_str())
    .bind(owner.map(|owner| i64::from(owner.pid)))
    .bind(owner.and_then(|owner| owner.start))
    .execute(conn)
    .await
    .context("writing the Environment to the registry failed")?;
    Ok(())
}

pub(super) async fn set_driver_metadata(
    conn: &mut SqliteConnection,
    id: &str,
    metadata: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let updated = sqlx::query("update environments set driver_metadata = ? where id = ?")
        .bind(serde_json::to_string(metadata)?)
        .bind(id)
        .execute(conn)
        .await
        .context("writing the Driver metadata failed")?;
    if updated.rows_affected() == 0 {
        bail!("Environment {id} is not in the registry");
    }
    Ok(())
}

pub(super) async fn set_claim(
    conn: &mut SqliteConnection,
    id: &str,
    owner: Option<Process>,
    destroying: bool,
) -> anyhow::Result<()> {
    sqlx::query(
        "update environments set owner_pid = ?, owner_pid_start = ?, destroying = ? where id = ?",
    )
    .bind(owner.map(|owner| i64::from(owner.pid)))
    .bind(owner.and_then(|owner| owner.start))
    .bind(destroying)
    .bind(id)
    .execute(conn)
    .await
    .context("writing the claim of the Environment failed")?;
    Ok(())
}

pub(super) async fn delete_environment(
    conn: &mut SqliteConnection,
    id: &str,
) -> anyhow::Result<()> {
    sqlx::query("delete from environments where id = ?")
        .bind(id)
        .execute(conn)
        .await
        .context("deleting the Environment from the registry failed")?;
    Ok(())
}

pub(super) async fn apt_updated_at(
    conn: &mut SqliteConnection,
    id: &str,
) -> anyhow::Result<Option<SystemTime>> {
    let seconds: Option<Option<i64>> =
        sqlx::query_scalar("select apt_updated_at from environments where id = ?")
            .bind(id)
            .fetch_optional(conn)
            .await
            .context("reading the apt upgrade time failed")?;
    Ok(seconds
        .flatten()
        .map(|seconds| UNIX_EPOCH + Duration::from_secs(seconds as u64)))
}

pub(super) async fn set_apt_updated_at(
    conn: &mut SqliteConnection,
    id: &str,
    time: SystemTime,
) -> anyhow::Result<()> {
    sqlx::query("update environments set apt_updated_at = ? where id = ?")
        .bind(unix_time(time))
        .bind(id)
        .execute(conn)
        .await
        .context("writing the apt upgrade time failed")?;
    Ok(())
}

pub(super) async fn attachments(
    conn: &mut SqliteConnection,
    environment_id: &str,
) -> anyhow::Result<Vec<Attachment>> {
    let rows: Vec<(i64, i64, Option<i64>)> =
        sqlx::query_as("select id, pid, pid_start from attachments where environment_id = ?")
            .bind(environment_id)
            .fetch_all(conn)
            .await
            .context("reading the Attachments failed")?;
    Ok(rows
        .into_iter()
        .map(|(id, pid, start)| Attachment {
            id,
            process: Process {
                pid: pid as u32,
                start,
            },
        })
        .collect())
}

pub(super) async fn insert_attachment(
    conn: &mut SqliteConnection,
    environment_id: &str,
    process: Process,
) -> anyhow::Result<i64> {
    let inserted =
        sqlx::query("insert into attachments (environment_id, pid, pid_start) values (?, ?, ?)")
            .bind(environment_id)
            .bind(i64::from(process.pid))
            .bind(process.start)
            .execute(conn)
            .await
            .context("recording the Attachment failed")?;
    Ok(inserted.last_insert_rowid())
}

pub(super) async fn delete_attachment(conn: &mut SqliteConnection, id: i64) -> anyhow::Result<()> {
    sqlx::query("delete from attachments where id = ?")
        .bind(id)
        .execute(conn)
        .await
        .context("removing the Attachment failed")?;
    Ok(())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn unix_time(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(sqlx::FromRow)]
struct EnvironmentRow {
    driver: String,
    package_name: String,
    package_identifier: String,
    source_dir: String,
    root_dir: String,
    distro_family: String,
    distro_codename: String,
    distro_version: String,
    distro_is_devel: bool,
    persistence: String,
    purpose: String,
    owner_pid: Option<i64>,
    owner_pid_start: Option<i64>,
    destroying: bool,
    driver_metadata: String,
}

impl TryFrom<EnvironmentRow> for RegisteredEnvironment {
    type Error = anyhow::Error;

    fn try_from(row: EnvironmentRow) -> anyhow::Result<Self> {
        Ok(Self {
            environment: Environment {
                driver: DriverType::from_name(&row.driver)?,
                package_name: row.package_name,
                package_identifier: row.package_identifier,
                source_dir: PathBuf::from(row.source_dir),
                root_dir: PathBuf::from(row.root_dir),
                distro: DistroVersion {
                    distro: Distro::parse(&row.distro_family),
                    codename: row.distro_codename,
                    version: row.distro_version,
                    is_devel: row.distro_is_devel,
                },
                persistence: Persistence::from_name(&row.persistence)?,
                purpose: EnvironmentPurpose::from_name(&row.purpose)?,
            },
            driver_metadata: serde_json::from_str(&row.driver_metadata)
                .context("parsing the Driver metadata failed")?,
            owner: row.owner_pid.map(|pid| Process {
                pid: pid as u32,
                start: row.owner_pid_start,
            }),
            destroying: row.destroying,
        })
    }
}

#[derive(sqlx::FromRow)]
struct InvocationRow {
    id: i64,
    kind: String,
    source_dir: String,
    package_name: String,
    package_version: String,
    distro_family: String,
    distro_codename: String,
    distro_version: String,
    distro_is_devel: bool,
    driver: Option<String>,
    success: bool,
    changes_path: Option<String>,
    environment_id: Option<String>,
}

impl TryFrom<InvocationRow> for RecordedInvocation {
    type Error = anyhow::Error;

    fn try_from(row: InvocationRow) -> anyhow::Result<Self> {
        Ok(Self {
            kind: InvocationKind::from_name(&row.kind)?,
            source_dir: PathBuf::from(row.source_dir),
            package_name: row.package_name,
            package_version: row.package_version,
            distro: DistroVersion {
                distro: Distro::parse(&row.distro_family),
                codename: row.distro_codename,
                version: row.distro_version,
                is_devel: row.distro_is_devel,
            },
            driver: row
                .driver
                .as_deref()
                .map(DriverType::from_name)
                .transpose()?,
            success: row.success,
            changes_path: row.changes_path.map(PathBuf::from),
            environment_id: row.environment_id,
        })
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(in crate::environment) async fn temp_registry() -> (PathBuf, Registry) {
        let dir = std::env::temp_dir().join(format!("debmagic-registry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let registry = Registry::open(&dir.join("db.sqlite")).await.unwrap();
        (dir, registry)
    }

    pub(in crate::environment) fn sample_environment(
        source_dir: &Path,
        purpose: EnvironmentPurpose,
    ) -> Environment {
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

    pub(in crate::environment) async fn upsert(
        registry: &Registry,
        environment: &Environment,
        owner: Option<Process>,
    ) {
        let mut conn = registry.connection().await.unwrap();
        upsert_environment(&mut conn, environment, owner)
            .await
            .unwrap();
    }

    fn invocation(
        kind: InvocationKind,
        source_dir: &Path,
        version: &str,
        distro: &DistroVersion,
        success: bool,
        changes_path: Option<&Path>,
    ) -> RecordedInvocation {
        RecordedInvocation {
            kind,
            source_dir: source_dir.to_path_buf(),
            package_name: "pkg".to_string(),
            package_version: version.to_string(),
            distro: distro.clone(),
            driver: Some(DriverType::Docker),
            success,
            changes_path: changes_path.map(Path::to_path_buf),
            environment_id: None,
        }
    }

    const BINARY: InvocationKind = InvocationKind::Build(BuildKind::Binary);

    async fn invocation_count(registry: &Registry) -> i64 {
        sqlx::query_scalar("select count(*) from invocations")
            .fetch_one(&mut *registry.connection().await.unwrap())
            .await
            .unwrap()
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

    #[tokio::test]
    async fn upsert_and_list_round_trip() -> anyhow::Result<()> {
        let (_dir, registry) = temp_registry().await;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        upsert(&registry, &environment, Some(Process::of(1))).await;
        let listed = registry.list().await?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].environment.id(), environment.id());
        assert_eq!(listed[0].environment.source_dir, PathBuf::from("/src/a"));
        assert_eq!(listed[0].environment.persistence, Persistence::Always);
        assert_eq!(listed[0].owner.map(|owner| owner.pid), Some(1));
        Ok(())
    }

    #[tokio::test]
    async fn unknown_enum_values_are_rejected() {
        let (_dir, registry) = temp_registry().await;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        upsert(&registry, &environment, None).await;
        let error = sqlx::query("update environments set purpose = 'deploy'")
            .execute(&mut *registry.connection().await.unwrap())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("CHECK"), "{error}");
    }

    #[tokio::test]
    async fn deleting_an_environment_drops_its_attachments_and_keeps_invocations()
    -> anyhow::Result<()> {
        let (_dir, registry) = temp_registry().await;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        let id = environment.id();
        upsert(&registry, &environment, None).await;
        let mut conn = registry.connection().await?;
        insert_attachment(&mut conn, &id, Process::current()).await?;
        drop(conn);
        let distro = DistroVersion::new(Distro::Debian, "trixie", "13");
        let mut recorded = invocation(
            InvocationKind::Test,
            Path::new("/src/a"),
            "1.0",
            &distro,
            true,
            None,
        );
        recorded.environment_id = Some(id.clone());
        registry.record_invocation(&recorded).await?;

        let mut conn = registry.connection().await?;
        delete_environment(&mut conn, &id).await?;
        assert!(attachments(&mut conn, &id).await?.is_empty());
        drop(conn);
        let invocations = registry.invocations_for_source(Path::new("/src/a")).await?;
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].environment_id, None);
        Ok(())
    }

    #[tokio::test]
    async fn invocation_records_full_distro_version() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry().await;
        let source = dir.join("src");
        let changes = dir.join("pkg.changes");
        std::fs::write(&changes, "changes")?;
        let distro = DistroVersion {
            distro: Distro::Custom("yocto".to_string()),
            codename: "kirkstone".to_string(),
            version: "4.0".to_string(),
            is_devel: true,
        };
        registry
            .record_invocation(&invocation(
                BINARY,
                &source,
                "1.0",
                &distro,
                true,
                Some(&changes),
            ))
            .await?;

        let found = registry.binary_build_changes_for_tree(&source).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].distro, distro);
        assert_eq!(found[0].kind, BINARY);
        Ok(())
    }

    #[tokio::test]
    async fn binary_changes_lookup_skips_missing_files() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry().await;
        let source = dir.join("src");
        let trixie = dir.join("trixie.changes");
        std::fs::write(&trixie, "changes")?;
        let missing = dir.join("gone.changes");
        let distro_trixie = DistroVersion::new(Distro::Debian, "trixie", "13");
        let distro_sid = DistroVersion::new(Distro::Debian, "sid", "");
        for (distro, changes) in [
            (&distro_trixie, &missing),
            (&distro_trixie, &trixie),
            (&distro_sid, &trixie),
        ] {
            registry
                .record_invocation(&invocation(
                    BINARY,
                    &source,
                    "1.0",
                    distro,
                    true,
                    Some(changes),
                ))
                .await?;
        }

        assert_eq!(invocation_count(&registry).await, 2);
        let found = registry.binary_build_changes_for_tree(&source).await?;
        let codenames: Vec<_> = found.iter().map(|i| i.distro.codename.as_str()).collect();
        assert_eq!(codenames, ["sid", "trixie"]);
        Ok(())
    }

    #[tokio::test]
    async fn invocation_history_keeps_the_newest_usable_binary_build() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry().await;
        let source = dir.join("src");
        let older = dir.join("older.changes");
        let newer = dir.join("newer.changes");
        std::fs::write(&older, "old")?;
        std::fs::write(&newer, "new")?;
        let distro = DistroVersion::new(Distro::Debian, "trixie", "13");

        registry
            .record_invocation(&invocation(
                BINARY,
                &source,
                "1.0",
                &distro,
                true,
                Some(&older),
            ))
            .await?;
        let mut lxd = invocation(BINARY, &source, "1.1", &distro, true, Some(&newer));
        lxd.driver = Some(DriverType::Lxd);
        registry.record_invocation(&lxd).await?;
        registry
            .record_invocation(&invocation(BINARY, &source, "1.2", &distro, false, None))
            .await?;

        assert_eq!(invocation_count(&registry).await, 1);
        let found = registry.binary_build_changes_for_tree(&source).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].changes_path.as_deref(), Some(newer.as_path()));
        assert_eq!(found[0].driver, Some(DriverType::Lxd));
        assert_eq!(found[0].package_version, "1.1");
        Ok(())
    }

    #[tokio::test]
    async fn deleting_a_changes_file_prunes_that_invocation() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry().await;
        let source = dir.join("src");
        let changes = dir.join("pkg.changes");
        std::fs::write(&changes, "changes")?;
        let distro = DistroVersion::new(Distro::Debian, "trixie", "13");
        registry
            .record_invocation(&invocation(
                BINARY,
                &source,
                "1.0",
                &distro,
                true,
                Some(&changes),
            ))
            .await?;
        std::fs::remove_file(&changes)?;

        assert!(
            registry
                .binary_build_changes_for_tree(&source)
                .await?
                .is_empty()
        );
        assert_eq!(invocation_count(&registry).await, 0);
        Ok(())
    }

    #[tokio::test]
    async fn non_binary_invocations_collapse_to_the_newest() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry().await;
        let source = dir.join("src");
        let distro = DistroVersion::new(Distro::Debian, "trixie", "13");
        for version in ["1.0", "1.1"] {
            registry
                .record_invocation(&invocation(
                    InvocationKind::Test,
                    &source,
                    version,
                    &distro,
                    true,
                    None,
                ))
                .await?;
        }
        let rows = registry.invocations_for_source(&source).await?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].package_version, "1.1");
        Ok(())
    }

    #[tokio::test]
    async fn apt_updated_at_roundtrips() -> anyhow::Result<()> {
        let (_dir, registry) = temp_registry().await;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        let id = environment.id();
        upsert(&registry, &environment, None).await;
        let mut conn = registry.connection().await?;
        assert_eq!(apt_updated_at(&mut conn, &id).await?, None);

        let time = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        set_apt_updated_at(&mut conn, &id, time).await?;
        assert_eq!(apt_updated_at(&mut conn, &id).await?, Some(time));
        // a re-claim keeps the record
        upsert_environment(&mut conn, &environment, None).await?;
        assert_eq!(apt_updated_at(&mut conn, &id).await?, Some(time));
        Ok(())
    }

    #[tokio::test]
    async fn open_if_exists_does_not_create_the_registry() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!(
            "debmagic-missing-{}/db.sqlite",
            uuid::Uuid::new_v4()
        ));
        assert!(Registry::open_if_exists(&path).await?.is_none());
        assert!(!path.parent().unwrap().exists());
        Ok(())
    }

    #[tokio::test]
    async fn reopening_keeps_the_rows() -> anyhow::Result<()> {
        let (dir, registry) = temp_registry().await;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        upsert(&registry, &environment, None).await;
        registry.pool.close().await;

        let registry = Registry::open(&dir.join("db.sqlite")).await?;
        let listed = registry.list().await?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].environment.id(), environment.id());
        Ok(())
    }

    async fn add_future_migration(path: &Path) {
        let registry = Registry::open(path).await.unwrap();
        sqlx::query(
            "insert into _sqlx_migrations (version, description, success, checksum, execution_time)
             values (99, 'future', 1, x'00', 0)",
        )
        .execute(&registry.pool)
        .await
        .unwrap();
        registry.pool.close().await;
    }

    #[tokio::test]
    async fn newer_schema_is_refused() {
        let (dir, registry) = temp_registry().await;
        registry.pool.close().await;
        let path = dir.join("db.sqlite");
        add_future_migration(&path).await;

        let Err(error) = Registry::open(&path).await else {
            panic!("expected a newer schema to be refused");
        };
        let rendered = format!("{error:#}");
        assert!(rendered.contains("upgrade debmagic"), "{rendered}");
        assert!(rendered.contains("99"), "{rendered}");

        let Err(error) = Registry::open_or_ephemeral(&path).await else {
            panic!("expected the fallback not to hide a newer schema");
        };
        assert!(is_incompatible_schema(&error), "{error:#}");
    }

    #[tokio::test]
    async fn open_or_ephemeral_falls_back_when_the_file_is_unusable() -> anyhow::Result<()> {
        let (dir, _registry) = temp_registry().await;
        // a directory is not a database
        let registry = Registry::open_or_ephemeral(&dir).await?;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        upsert(&registry, &environment, None).await;
        assert_eq!(registry.list().await?.len(), 1);
        Ok(())
    }

    /// Each opener gets its own runtime, like separate debmagic processes.
    #[test]
    fn concurrent_first_opens_all_succeed() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("debmagic-registry-{}", uuid::Uuid::new_v4()));
        let path = dir.join("db.sqlite");
        let barrier = std::sync::Barrier::new(8);
        std::thread::scope(|scope| {
            let openers: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        let runtime = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()?;
                        barrier.wait();
                        runtime.block_on(Registry::open(&path)).map(|_| ())
                    })
                })
                .collect();
            openers
                .into_iter()
                .try_for_each(|opener| opener.join().expect("opener thread panicked"))
        })
    }
}
