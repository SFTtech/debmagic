mod process;
mod registry;

pub use registry::{InvocationKind, RecordedInvocation, RegisteredEnvironment, Registry};

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, bail};
use process::Process;
use sqlx::sqlite::SqliteConnection;

use crate::apt::AptConfig;
use crate::driver::{
    Driver, Environment, EnvironmentDriver, EnvironmentMetadata, Persistence, ResourceStatus,
    config::{DriverConfig, DriverOverrides},
    create_driver_from_metadata, planned_driver_metadata, remove_environment_root,
};

/// How [`ClaimedEnvironment::acquire`] treats the Host root of an Environment
/// it reuses (Persistence `on-failure` or `always`). A Host root that is not
/// reused always starts empty.
pub enum HostRootPolicy {
    Reset,
    /// Keep the tree when `manifest` exists in it, so an Incremental build
    /// continues from the previous run's state.
    KeepIncremental {
        manifest: std::path::PathBuf,
    },
    /// Keep the whole Host root. The caller has already decided an incremental
    /// manifest for this run exists.
    Keep,
    /// Delete only this stage directory, so a sibling build kind sharing the
    /// Environment keeps its tree.
    ResetStage {
        stage_dir: std::path::PathBuf,
    },
}

/// An Environment this process has claimed in the Environment registry, with
/// its Host root prepared and its Driver created. Only [`claim_command`]
/// hands one out, and it always finishes the claim.
pub struct ClaimedEnvironment<'a> {
    registry: &'a Registry,
    driver_config: &'a DriverConfig,
    environment: Environment,
    driver: Driver,
    /// `on-failure` keeps the Environment only when the build or TestRun
    /// command itself failed. Set before [`Self::finish`].
    keep_after_failure: bool,
}

impl<'a> ClaimedEnvironment<'a> {
    /// Claim `environment`, prepare its Host root, run `stage` to fill it
    /// (after the Environment directories exist), and create its Driver.
    ///
    /// A fresh Host root is staged before the Driver is created, because
    /// container Drivers bind-mount it. A reused one gets its Driver first,
    /// so the Driver can reset the root from inside the Environment.
    async fn acquire(
        registry: &'a Registry,
        environment: Environment,
        driver_config: &'a DriverConfig,
        apt: &AptConfig,
        overrides: &DriverOverrides,
        root_policy: HostRootPolicy,
        stage: impl FnOnce(&Environment, Option<&Driver>) -> anyhow::Result<()>,
    ) -> anyhow::Result<Self> {
        let planned = planned_driver_metadata(&environment, driver_config, overrides);
        let previous = begin_environment(registry, &environment, Process::current(), &planned)
            .await?
            .map(|row| row.metadata());

        if environment.persistence.reuses() && environment.root_dir.exists() {
            let claimed =
                Self::create_driver(registry, environment, driver_config, apt, overrides).await?;
            if let Err(error) = claimed.prepare_reused_root(root_policy, stage) {
                return claimed.abort(error).await;
            }
            return Ok(claimed);
        }

        let prepared =
            remove_environment_root(&environment.root_dir, driver_config, previous.as_ref())
                .and_then(|()| {
                    environment
                        .create_dirs()
                        .context("failed to create Environment directories")
                })
                .and_then(|()| stage(&environment, None));
        if let Err(error) = prepared {
            abandon_claim(registry, &environment, driver_config, false).await;
            return Err(error);
        }
        Self::create_driver(registry, environment, driver_config, apt, overrides).await
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn driver(&self) -> &Driver {
        &self.driver
    }

    async fn finish(self) -> anyhow::Result<()> {
        finish_environment(
            self.registry,
            &self.environment,
            &self.driver,
            self.driver_config,
            self.keep_after_failure,
        )
        .await
    }

    /// Finish after `error` stopped the run, keeping `error` as the result.
    async fn abort<T>(self, error: anyhow::Error) -> anyhow::Result<T> {
        let id = self.environment.id();
        if let Err(teardown) = self.finish().await {
            eprintln!("Warning: failed to tear down Environment {id}: {teardown:#}");
        }
        Err(error)
    }

    async fn create_driver(
        registry: &'a Registry,
        environment: Environment,
        driver_config: &'a DriverConfig,
        apt: &AptConfig,
        overrides: &DriverOverrides,
    ) -> anyhow::Result<Self> {
        let driver = match crate::driver::create_driver(&environment, driver_config, apt, overrides)
        {
            Ok(driver) => driver,
            Err(error) => {
                abandon_claim(registry, &environment, driver_config, true).await;
                return Err(
                    error.context(format!("failed to create {:?} driver", environment.driver))
                );
            }
        };
        // Best effort: the claim already recorded the planned Driver metadata,
        // which is enough to find the resource.
        if let Err(error) = record_driver_metadata(registry, &environment, &driver).await {
            eprintln!("Warning: failed to record Driver metadata: {error:#}");
        }
        let claimed = Self {
            registry,
            driver_config,
            environment,
            driver,
            keep_after_failure: false,
        };
        if let Err(error) = claimed.upgrade_packages(apt.update_age).await {
            return claimed
                .abort(error.context("upgrading the packages in the Environment failed"))
                .await;
        }
        Ok(claimed)
    }

    /// Upgrade the packages of a container Environment unless `policy` says
    /// the last upgrade is recent enough: the one recorded in the registry
    /// for a reused Environment, the image's for a fresh one. A fresh one
    /// without a refreshed image always upgrades. The Bare driver is the
    /// host, which the user maintains.
    async fn upgrade_packages(&self, policy: crate::time::RefreshPolicy) -> anyhow::Result<()> {
        if self.environment.driver == crate::driver::DriverType::Bare {
            return Ok(());
        }
        let id = self.environment.id();
        let reused = self.driver.reused_environment();
        let last_update = if reused {
            registry::apt_updated_at(&mut *self.registry.connection().await?, &id).await?
        } else {
            self.driver.packages_refreshed_at()
        };
        let current = (reused || last_update.is_some())
            && !crate::driver::apt_update_needed_on_reuse(policy, last_update);
        if current && reused {
            return Ok(());
        }
        // record a fresh Environment's image time for its reuse
        let updated_at = match last_update.filter(|_| current) {
            Some(image_refreshed_at) => image_refreshed_at,
            None => {
                crate::output::step("Upgrading the packages in the Environment");
                crate::apt::refresh(&self.driver, &self.environment.root_dir)?;
                std::time::SystemTime::now()
            }
        };
        let recorded = async {
            let mut conn = self.registry.connection().await?;
            registry::set_apt_updated_at(&mut conn, &id, updated_at).await
        };
        if let Err(error) = recorded.await {
            eprintln!("Warning: failed to record the apt upgrade time: {error:#}");
        }
        Ok(())
    }

    fn prepare_reused_root(
        &self,
        root_policy: HostRootPolicy,
        stage: impl FnOnce(&Environment, Option<&Driver>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        enum Action {
            Reset,
            Keep,
            ResetStage(std::path::PathBuf),
        }
        let action = match root_policy {
            HostRootPolicy::Reset => Action::Reset,
            HostRootPolicy::KeepIncremental { manifest } => {
                if manifest.is_file() {
                    Action::Keep
                } else {
                    Action::Reset
                }
            }
            HostRootPolicy::Keep => Action::Keep,
            HostRootPolicy::ResetStage { stage_dir } => Action::ResetStage(stage_dir),
        };
        match action {
            Action::Reset => {
                self.driver
                    .reset_root()
                    .context("failed to reset the Host root of the reused Environment")?;
            }
            Action::ResetStage(stage_dir) => {
                if stage_dir.exists() {
                    self.driver
                        .run_command_checked(
                            &["find", ".", "-mindepth", "1", "-delete"],
                            &stage_dir,
                            true,
                            &[],
                        )
                        .context("failed to reset the source stage directory")?;
                }
            }
            Action::Keep => {
                if !self.driver.reused_environment() {
                    println!("Keeping incremental build tree in a fresh build environment");
                }
            }
        }
        self.environment
            .create_dirs()
            .context("failed to create Environment directories")?;
        stage(&self.environment, Some(&self.driver))
    }
}

async fn record_driver_metadata(
    registry: &Registry,
    environment: &Environment,
    driver: &Driver,
) -> anyhow::Result<()> {
    let mut conn = registry.connection().await?;
    registry::set_driver_metadata(&mut conn, &environment.id(), &driver.driver_metadata()).await
}

/// What a command finished with, once an Environment is claimed.
///
/// `Err` from the command callback is not a completion: nothing is recorded,
/// and the Environment is finished with a warning on failure.
pub struct CommandCompletion<T> {
    /// Returned after the Invocation is recorded and the claim is finished.
    /// A failed build is a completion whose value is `Err`.
    pub value: anyhow::Result<T>,
    /// Whether the Invocation succeeded.
    pub success: bool,
    /// Whether the build or TestRun command itself failed. Persistence
    /// `on-failure` keeps the Environment, and the inspection hint is printed.
    /// Export, signing, and a strict skip are not this.
    pub shell_worthy: bool,
    /// Whether a finish error fails the command. Otherwise it is a warning
    /// and `value` is still returned.
    pub finish_error_fails_command: bool,
    /// Exported `.changes` for a successful binary build. Absent for a source
    /// build, a TestRun, and a failed binary build.
    pub changes_path: Option<PathBuf>,
    /// Lines printed before the inspection hint when `shell_worthy`.
    pub failure_lines: Vec<String>,
}

/// Hint printed when a failed build or TestRun kept its Environment.
pub fn inspection_hint(environment_id: &str) -> String {
    format!("Get a shell with `debmagic env shell {environment_id}`")
}

/// Claim `environment`, stage it, run `command`, and on a completion record
/// the Invocation, maybe print the inspection hint, and finish.
///
/// The caller builds the Environment and opens the registry. Distro, Driver,
/// Environment id, package name, and source directory on the Invocation come
/// from the Environment.
#[allow(clippy::too_many_arguments)]
pub async fn claim_command<T>(
    registry: &Registry,
    environment: Environment,
    driver_config: &DriverConfig,
    apt: &AptConfig,
    overrides: &DriverOverrides,
    root_policy: HostRootPolicy,
    stage: impl FnOnce(&Environment, Option<&Driver>) -> anyhow::Result<()>,
    kind: InvocationKind,
    package_version: &str,
    command: impl FnOnce(&ClaimedEnvironment<'_>) -> anyhow::Result<CommandCompletion<T>>,
) -> anyhow::Result<T> {
    let mut claimed = ClaimedEnvironment::acquire(
        registry,
        environment,
        driver_config,
        apt,
        overrides,
        root_policy,
        stage,
    )
    .await?;
    let completion = match command(&claimed) {
        Ok(completion) => completion,
        Err(error) => return claimed.abort(error).await,
    };

    let environment = claimed.environment();
    let environment_id = environment.id();
    let purpose = environment.purpose.as_str();
    let invocation = RecordedInvocation {
        kind,
        source_dir: environment.source_dir.clone(),
        package_name: environment.package_name.clone(),
        package_version: package_version.to_string(),
        distro: environment.distro.clone(),
        driver: Some(environment.driver),
        success: completion.success,
        changes_path: completion.changes_path.clone(),
        environment_id: Some(environment_id.clone()),
    };
    if let Err(error) = registry.record_invocation(&invocation).await {
        eprintln!("Warning: failed to record Invocation: {error:#}");
    }

    if completion.shell_worthy {
        for line in &completion.failure_lines {
            eprintln!("{line}");
        }
        if environment.persistence.reuses() {
            eprintln!("{}", inspection_hint(&environment_id));
            claimed.keep_after_failure = true;
        }
    }

    let finish_error_fails = completion.finish_error_fails_command;
    let value = completion.value;
    match claimed.finish().await {
        Ok(()) => value,
        Err(error) if finish_error_fails => {
            Err(error.context(format!("failed to clean up {purpose} environment")))
        }
        Err(error) => {
            eprintln!("Failed to clean up {purpose} environment: {error:#}");
            value
        }
    }
}

/// Register `environment` as claimed by `owner` with the Driver metadata
/// planned for it, refusing when a live different process already owns it
/// or a live Attachment is open on it (the run would reset the Host root, or
/// replace the container, under the shell).
///
/// Returns the row as it was before, so the caller can still reach a Driver
/// resource recorded under the previous metadata.
async fn begin_environment(
    registry: &Registry,
    environment: &Environment,
    owner: Process,
    planned_driver_metadata: &HashMap<String, String>,
) -> anyhow::Result<Option<RegisteredEnvironment>> {
    let id = environment.id();
    let mut tx = registry.write().await?;
    let previous = registry::get_environment(&mut tx, &id).await?;
    if let Some(previous) = &previous {
        if let Some(holder) = previous.owner
            && holder.pid != owner.pid
            && holder.is_alive()
        {
            bail!(
                "Environment {id} is already in use by process {}",
                holder.pid
            );
        }
        let attachments = live_attachment_count(&mut tx, &id).await?;
        if attachments > 0 {
            bail!(
                "Environment {id} has {attachments} live Attachment(s) \
                 (`debmagic env shell`); exit them first"
            );
        }
    }
    registry::upsert_environment(&mut tx, environment, Some(owner)).await?;
    registry::set_driver_metadata(&mut tx, &id, planned_driver_metadata).await?;
    tx.commit().await?;
    Ok(previous)
}

/// Verify that an Environment has no live Attachments and no live owner,
/// then mark it as being destroyed by this process, returning its row.
/// Until released or deleted, this blocks new `env shell` Attachments
/// (refused while `destroying`) and `begin_environment` (live owner).
async fn claim_for_destroy(registry: &Registry, id: &str) -> anyhow::Result<RegisteredEnvironment> {
    let me = Process::current();
    let mut tx = registry.write().await?;
    let row = registry::get_environment(&mut tx, id)
        .await?
        .with_context(|| format!("no Environment with id {id}"))?;
    if live_attachment_count(&mut tx, id).await? > 0 {
        bail!("Environment {id} has live Attachments; stop them first");
    }
    if let Some(holder) = row.owner
        && holder.pid != me.pid
        && holder.is_alive()
    {
        if row.destroying {
            bail!(
                "Environment {id} is already being destroyed by process {}",
                holder.pid
            );
        }
        bail!("Environment {id} still has a creating command running");
    }
    registry::set_claim(&mut tx, id, Some(me), true).await?;
    tx.commit().await?;
    Ok(row)
}

async fn owned_by_me(conn: &mut SqliteConnection, id: &str) -> anyhow::Result<bool> {
    let me = Process::current();
    Ok(registry::get_environment(conn, id)
        .await?
        .and_then(|row| row.owner)
        .is_some_and(|owner| owner.pid == me.pid))
}

/// Drop this process's hold on an Environment, if it still has it.
async fn release(registry: &Registry, id: &str) -> anyhow::Result<()> {
    let mut tx = registry.write().await?;
    if owned_by_me(&mut tx, id).await? {
        registry::set_claim(&mut tx, id, None, false).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Delete the registry row, if this process still holds it.
async fn delete_if_owned(registry: &Registry, id: &str) -> anyhow::Result<()> {
    let mut tx = registry.write().await?;
    if owned_by_me(&mut tx, id).await? {
        registry::delete_environment(&mut tx, id).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Live Attachments of an Environment, deleting those whose shell is gone.
async fn live_attachment_count(conn: &mut SqliteConnection, id: &str) -> anyhow::Result<usize> {
    let mut live = 0;
    for attachment in registry::attachments(&mut *conn, id).await? {
        if attachment.process.is_alive() {
            live += 1;
        } else {
            registry::delete_attachment(&mut *conn, attachment.id).await?;
        }
    }
    Ok(live)
}

async fn reap_attachments(registry: &Registry, id: &str) -> anyhow::Result<usize> {
    let mut tx = registry.write().await?;
    let live = live_attachment_count(&mut tx, id).await?;
    tx.commit().await?;
    Ok(live)
}

/// Wait until no live Attachments remain. With `mark_destroying`, mark the
/// Environment as being destroyed in that same transaction, so no new
/// Attachment slips in before the teardown.
async fn wait_for_detach(
    registry: &Registry,
    id: &str,
    mark_destroying: bool,
) -> anyhow::Result<()> {
    let mut announced = false;
    loop {
        let mut tx = registry.write().await?;
        let live = live_attachment_count(&mut tx, id).await?;
        if live == 0 && mark_destroying {
            if !owned_by_me(&mut tx, id).await? {
                bail!("Environment {id} is no longer owned by this process");
            }
            registry::set_claim(&mut tx, id, Some(Process::current()), true).await?;
        }
        tx.commit().await?;
        if live == 0 {
            return Ok(());
        }
        if !announced {
            println!("Waiting for all attached shells to exit...");
            announced = true;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Attach `process` to an Environment, refusing one that is gone or being
/// destroyed.
async fn attach(registry: &Registry, id: &str, process: Process) -> anyhow::Result<i64> {
    let mut tx = registry.write().await?;
    match registry::get_environment(&mut tx, id).await? {
        Some(row) if !row.destroying => {}
        _ => bail!("Environment {id} is gone or being destroyed"),
    }
    let attachment = registry::insert_attachment(&mut tx, id, process).await?;
    tx.commit().await?;
    Ok(attachment)
}

/// Undo a claim when acquiring failed before a Driver existed. A failed
/// Driver creation may have left a resource under the planned metadata, so
/// then the row stays (unowned, hence Stale) for `debmagic env clean`.
async fn abandon_claim(
    registry: &Registry,
    environment: &Environment,
    driver_config: &DriverConfig,
    driver_attempted: bool,
) {
    let id = environment.id();
    let destroy = environment.persistence != Persistence::Always
        && !driver_attempted
        && match remove_environment_root(&environment.root_dir, driver_config, None) {
            Ok(()) => true,
            Err(error) => {
                eprintln!("Warning: failed to remove the Host root of {id}: {error:#}");
                false
            }
        };
    let released = if destroy {
        delete_if_owned(registry, &id).await
    } else {
        release(registry, &id).await
    };
    if let Err(error) = released {
        eprintln!("Warning: failed to release Environment {id} in the registry: {error:#}");
    }
}

async fn finish_environment(
    registry: &Registry,
    environment: &Environment,
    driver: &Driver,
    driver_config: &DriverConfig,
    keep_after_failure: bool,
) -> anyhow::Result<()> {
    let id = environment.id();
    let keep = match environment.persistence {
        Persistence::Always => true,
        Persistence::OnFailure => keep_after_failure,
        Persistence::No => false,
    };
    if keep {
        wait_for_detach(registry, &id, false).await?;
        return release(registry, &id).await;
    }

    // Keep ownership while destroying: concurrent `begin_environment` calls
    // see a live owner and refuse, and marking `destroying` blocks new
    // Attachments, so nothing can start using the Environment mid-teardown.
    wait_for_detach(registry, &id, true).await?;
    let metadata = EnvironmentMetadata {
        environment: environment.clone(),
        driver_metadata: driver.driver_metadata(),
    };
    destroy_claimed(registry, &metadata, Some(driver), driver_config, false).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentStatus {
    Live,
    Stale,
    Unreachable,
}

impl EnvironmentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Stale => "stale",
            Self::Unreachable => "unreachable",
        }
    }
}

/// Live, Stale, or Unreachable, plus the live Attachment count. Assessing
/// reaps dead Attachments.
pub struct AssessedEnvironment {
    pub status: EnvironmentStatus,
    pub attachments: usize,
}

/// Derive whether a registered Environment is Live, Stale, or Unreachable,
/// and how many live Attachments it has.
pub async fn assess_environment(
    registry: &Registry,
    registered: &RegisteredEnvironment,
    driver_config: &DriverConfig,
) -> anyhow::Result<AssessedEnvironment> {
    // Fail closed: a registry error must never look like "no Attachments",
    // since this status drives destruction.
    let attachments = reap_attachments(registry, &registered.environment.id()).await?;
    let leftover = registered.environment.persistence == Persistence::No
        && registered.owner.is_none_or(|owner| !owner.is_alive())
        && attachments == 0;
    let root_gone = !registered.environment.root_dir.exists();

    let status = match create_driver_from_metadata(driver_config, &registered.metadata()) {
        Err(_) if leftover || root_gone => EnvironmentStatus::Stale,
        Err(_) => EnvironmentStatus::Unreachable,
        Ok(driver) => match driver.probe_resource() {
            ResourceStatus::Unreachable => EnvironmentStatus::Unreachable,
            ResourceStatus::Absent => EnvironmentStatus::Stale,
            ResourceStatus::Present if leftover || root_gone => EnvironmentStatus::Stale,
            ResourceStatus::Present => EnvironmentStatus::Live,
        },
    };
    Ok(AssessedEnvironment {
        status,
        attachments,
    })
}

/// Open a shell on an Environment that is already chosen and Live. Holds an
/// Attachment for the duration of the shell and releases it afterwards.
pub async fn shell_environment(
    registry: &Registry,
    driver_config: &DriverConfig,
    registered: &RegisteredEnvironment,
) -> anyhow::Result<()> {
    shell_while_attached(registry, driver_config, registered, open_interactive_shell).await
}

/// Assess, hold an Attachment around `during`, then release it. `during` is
/// the interactive shell in production. Tests pass their own check so they
/// exercise this same path without spawning `$SHELL` on the terminal.
async fn shell_while_attached(
    registry: &Registry,
    driver_config: &DriverConfig,
    registered: &RegisteredEnvironment,
    during: impl FnOnce(&DriverConfig, &RegisteredEnvironment) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let id = registered.environment.id();
    match assess_environment(registry, registered, driver_config)
        .await?
        .status
    {
        EnvironmentStatus::Unreachable => bail!("Environment {id} is Unreachable"),
        EnvironmentStatus::Stale => {
            bail!("Environment {id} is Stale; run `debmagic env rm {id}`")
        }
        EnvironmentStatus::Live => {}
    }

    let attachment = attach(registry, &id, Process::current()).await?;
    let result = during(driver_config, registered);
    let detached =
        async { registry::delete_attachment(&mut *registry.connection().await?, attachment).await };
    result.and(detached.await)
}

fn open_interactive_shell(
    driver_config: &DriverConfig,
    registered: &RegisteredEnvironment,
) -> anyhow::Result<()> {
    let driver = create_driver_from_metadata(driver_config, &registered.metadata())?;
    driver
        .interactive_shell(&shell_cwd(&registered.environment))
        .map_err(|error| anyhow::anyhow!("interactive shell failed: {error}"))?;
    Ok(())
}

/// Where an interactive shell starts. Binary and source builds stage into
/// sibling trees; the binary tree is the iteration workflow, with the source
/// tree as fallback. Environments staged only by the Environment module
/// itself use [`Environment::staged_source_dir`].
fn shell_cwd(environment: &Environment) -> std::path::PathBuf {
    let binary = environment
        .work_dir()
        .join(format!("{}-binary", environment.package_identifier));
    let source = environment
        .work_dir()
        .join(format!("{}-source", environment.package_identifier));
    if binary.is_dir() {
        binary
    } else if source.is_dir() {
        source
    } else {
        environment.staged_source_dir()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestroyWhen {
    /// `env clean`: only what is still Stale once claimed.
    StillStale,
    /// `env rm <id>`: anything but Unreachable.
    Reachable,
    /// `env rm <id> --force`: also without Driver cooperation; Driver
    /// failures only warn.
    Always,
}

/// Claim the Environment for destruction (refusing live Attachments and a
/// live owner), then assess the row read by the claim: between the caller's
/// assessment and the claim, a run may have recreated it. Destroys the
/// Driver resource, Host root, and registry row when `when` still holds;
/// otherwise releases the claim and returns `false`.
///
/// The Driver commands run outside the claim's transaction, which would
/// otherwise lock every other debmagic out of the registry meanwhile; the
/// `destroying` mark keeps the Environment reserved instead.
pub async fn claim_and_destroy(
    registry: &Registry,
    driver_config: &DriverConfig,
    id: &str,
    when: DestroyWhen,
) -> anyhow::Result<bool> {
    let mut claimed = claim_for_destroy(registry, id).await?;
    // The claim verified the previous owner was gone; we are only holding
    // the row, which must not make it look in use.
    claimed.owner = None;
    let status = match assess_environment(registry, &claimed, driver_config).await {
        Ok(assessed) => assessed.status,
        Err(error) => {
            let _ = release(registry, id).await;
            return Err(error);
        }
    };
    let proceed = match when {
        DestroyWhen::StillStale => status == EnvironmentStatus::Stale,
        DestroyWhen::Reachable => status != EnvironmentStatus::Unreachable,
        DestroyWhen::Always => true,
    };
    if !proceed {
        release(registry, id).await?;
        return Ok(false);
    }

    let force = when == DestroyWhen::Always;
    let metadata = claimed.metadata();
    let driver = match create_driver_from_metadata(driver_config, &metadata) {
        Ok(driver) => Some(driver),
        Err(error) if force => {
            eprintln!(
                "Warning: cannot reattach to the Driver of {id} ({error}); its resource may leak"
            );
            None
        }
        Err(error) => {
            return Err(error.context(format!(
                "cannot reattach to the Driver of Environment {id}; \
                 pass --force to destroy it without Driver cooperation"
            )));
        }
    };
    destroy_claimed(registry, &metadata, driver.as_ref(), driver_config, force).await?;
    Ok(true)
}

/// Destroy an Environment this process has marked `destroying`: its Driver
/// resource, Host root, and registry row, in that order. Any failure keeps
/// the claim, so the row stays `destroying` (no new Attachments) and still
/// records the resource for a later `env rm`. With `force`, a failed
/// resource destroy only warns.
async fn destroy_claimed(
    registry: &Registry,
    metadata: &EnvironmentMetadata,
    driver: Option<&Driver>,
    driver_config: &DriverConfig,
    force: bool,
) -> anyhow::Result<()> {
    let id = metadata.environment.id();
    if let Some(driver) = driver {
        if let Err(error) = driver.reset_root() {
            eprintln!("Warning: failed to reset the Host root of {id} before destroy: {error}");
        }
        if let Err(error) = driver.destroy_resource() {
            if !force {
                return Err(error.context(format!(
                    "failed to destroy the Driver resource of Environment {id}"
                )));
            }
            eprintln!("Warning: failed to destroy the Driver resource of {id}: {error}");
        }
    }
    remove_environment_root(
        &metadata.environment.root_dir,
        driver_config,
        Some(metadata),
    )?;
    delete_if_owned(registry, &id).await
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
    let mut seen = std::collections::HashSet::new();
    invocations
        .iter()
        .filter(|invocation| {
            distro_filter.is_none_or(|codename| invocation.distro.codename == codename)
        })
        .filter(|invocation| {
            seen.insert((
                invocation.distro.distro.as_str().to_string(),
                invocation.distro.codename.clone(),
            ))
        })
        .collect()
}

/// 0 / 1 / 2+ DistroVersions among binary-build Invocations with existing `.changes`.
fn select_unique_binary_changes(
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
    use std::path::{Path, PathBuf};

    use debmagic_common::distro::{Distro, DistroVersion};

    use super::registry::tests::{sample_environment, temp_registry, upsert};
    use super::*;
    use crate::build_intent::BuildKind;
    use crate::driver::{DriverType, EnvironmentPurpose};

    fn dead_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    fn registered(environment: Environment) -> RegisteredEnvironment {
        RegisteredEnvironment {
            environment,
            driver_metadata: HashMap::new(),
            owner: None,
            destroying: false,
        }
    }

    #[tokio::test]
    async fn begin_environment_refuses_live_owner_and_allows_takeover() -> anyhow::Result<()> {
        let (_dir, registry) = temp_registry().await;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        let none = HashMap::new();

        // pid 1 is always alive; a different process must not take over.
        begin_environment(&registry, &environment, Process::of(1), &none).await?;
        let error = begin_environment(&registry, &environment, Process::current(), &none)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("already in use by process 1"), "{error}");

        // A dead owner can be taken over.
        upsert(&registry, &environment, Some(Process::of(dead_pid()))).await;
        begin_environment(&registry, &environment, Process::current(), &none).await?;

        // The same process re-beginning is idempotent.
        begin_environment(&registry, &environment, Process::current(), &none).await?;
        Ok(())
    }

    #[tokio::test]
    async fn begin_environment_refuses_live_attachments() -> anyhow::Result<()> {
        let (_dir, registry) = temp_registry().await;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        upsert(&registry, &environment, None).await;
        attach(&registry, &environment.id(), Process::current()).await?;

        let error = begin_environment(&registry, &environment, Process::current(), &HashMap::new())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("live Attachment"), "{error}");
        Ok(())
    }

    #[tokio::test]
    async fn begin_environment_records_planned_metadata_and_returns_previous() -> anyhow::Result<()>
    {
        let (_dir, registry) = temp_registry().await;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        let first = HashMap::from([("project".to_string(), "old".to_string())]);
        let second = HashMap::from([("project".to_string(), "new".to_string())]);

        let me = Process::current();
        assert!(
            begin_environment(&registry, &environment, me, &first)
                .await?
                .is_none()
        );
        let previous = begin_environment(&registry, &environment, me, &second)
            .await?
            .expect("previous row");
        assert_eq!(previous.driver_metadata, first);
        assert_eq!(
            registry
                .get(&environment.id())
                .await?
                .unwrap()
                .driver_metadata,
            second
        );
        Ok(())
    }

    #[tokio::test]
    async fn released_destroy_claim_allows_attachments_again() -> anyhow::Result<()> {
        let (_dir, registry) = temp_registry().await;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        let id = environment.id();
        upsert(&registry, &environment, None).await;
        claim_for_destroy(&registry, &id).await?;
        release(&registry, &id).await?;

        attach(&registry, &id, Process::current()).await?;
        assert_eq!(registry.get(&id).await?.unwrap().owner, None);
        Ok(())
    }

    #[tokio::test]
    async fn claim_for_destroy_blocks_attachments_and_delete_removes_row() -> anyhow::Result<()> {
        let (_dir, registry) = temp_registry().await;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        let id = environment.id();
        upsert(&registry, &environment, None).await;

        claim_for_destroy(&registry, &id).await?;
        let error = attach(&registry, &id, Process::current())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("gone or being destroyed"), "{error}");

        delete_if_owned(&registry, &id).await?;
        assert!(registry.get(&id).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn claim_for_destroy_refuses_live_attachments() -> anyhow::Result<()> {
        let (_dir, registry) = temp_registry().await;
        let environment = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        upsert(&registry, &environment, None).await;
        attach(&registry, &environment.id(), Process::current()).await?;

        let error = claim_for_destroy(&registry, &environment.id())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("live Attachments"), "{error}");
        Ok(())
    }

    #[test]
    fn unique_shell_selection_is_zero_one_many() {
        let build = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Build);
        let test = sample_environment(Path::new("/src/a"), EnvironmentPurpose::Test);
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
        let version = |identifier: &str| {
            registered(Environment::new(
                DriverType::Docker,
                "pkg",
                identifier,
                Path::new("/src/a"),
                DistroVersion::new(Distro::Debian, "trixie", "13"),
                Persistence::Always,
                EnvironmentPurpose::Build,
                Path::new("/envs"),
            ))
        };
        let both = vec![version("pkg-1.0"), version("pkg-1.1")];
        let picked = select_unique_environment(&both, "pkg-1.1").unwrap();
        assert_eq!(picked.environment.package_identifier, "pkg-1.1");

        // No Environment of the current version: fall back to all of them.
        let error = select_unique_environment(&both, "pkg-2.0")
            .unwrap_err()
            .to_string();
        assert!(error.contains("more than one Environment"));
    }

    fn recorded_binary(codename: &str, changes: &str) -> RecordedInvocation {
        RecordedInvocation {
            kind: InvocationKind::Build(BuildKind::Binary),
            source_dir: PathBuf::from("/src"),
            package_name: "pkg".to_string(),
            package_version: "1.0".to_string(),
            distro: DistroVersion::new(Distro::Debian, codename, "13"),
            driver: Some(DriverType::Docker),
            success: true,
            changes_path: Some(PathBuf::from(changes)),
            environment_id: None,
        }
    }

    #[test]
    fn binary_changes_selection_splits_on_distro() {
        let trixie = recorded_binary("trixie", "/out/trixie.changes");
        let older_trixie = recorded_binary("trixie", "/out/old.changes");
        let sid = recorded_binary("sid", "/out/sid.changes");
        let found = [trixie, older_trixie, sid];

        let error = select_unique_binary_changes(&found, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("more than one DistroVersion"));
        let picked = select_unique_binary_changes(&found, Some("trixie")).unwrap();
        assert_eq!(
            picked.changes_path.as_deref(),
            Some(Path::new("/out/trixie.changes"))
        );
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
}

#[cfg(test)]
mod teardown_tests {
    use std::path::PathBuf;

    use debmagic_common::distro::{Distro, DistroVersion};

    use super::registry::tests::upsert;
    use super::*;
    use crate::driver::{DriverType, EnvironmentPurpose};

    struct Fixture {
        dir: PathBuf,
        registry: Registry,
    }

    impl Fixture {
        async fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("debmagic-lifecycle-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let registry = Registry::open(&dir.join("db.sqlite")).await.unwrap();
            Self { dir, registry }
        }

        /// A registered Bare-driver Environment whose Host root exists.
        async fn environment(&self, persistence: Persistence, owner: Option<u32>) -> Environment {
            let environment = Environment::new(
                DriverType::Bare,
                "pkg",
                "pkg-1.0",
                &self.dir.join("src"),
                DistroVersion::new(Distro::Debian, "trixie", "13"),
                persistence,
                EnvironmentPurpose::Build,
                &self.dir.join("envs"),
            );
            environment.create_dirs().unwrap();
            upsert(&self.registry, &environment, owner.map(Process::of)).await;
            environment
        }

        async fn row(&self, environment: &Environment) -> Option<RegisteredEnvironment> {
            self.registry.get(&environment.id()).await.unwrap()
        }

        async fn assess(&self, environment: &Environment) -> AssessedEnvironment {
            let registered = self.row(environment).await.unwrap();
            assess_environment(&self.registry, &registered, &DriverConfig::default())
                .await
                .unwrap()
        }

        async fn claim_and_destroy(
            &self,
            environment: &Environment,
            when: DestroyWhen,
        ) -> anyhow::Result<bool> {
            claim_and_destroy(
                &self.registry,
                &DriverConfig::default(),
                &environment.id(),
                when,
            )
            .await
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn dead_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[tokio::test]
    async fn non_persistent_environment_with_dead_owner_is_stale() {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::No, Some(dead_pid())).await;
        assert_eq!(
            fixture.assess(&environment).await.status,
            EnvironmentStatus::Stale
        );
    }

    #[tokio::test]
    async fn non_persistent_environment_with_live_owner_is_live() {
        let fixture = Fixture::new().await;
        let environment = fixture
            .environment(Persistence::No, Some(std::process::id()))
            .await;
        assert_eq!(
            fixture.assess(&environment).await.status,
            EnvironmentStatus::Live
        );
    }

    #[tokio::test]
    async fn persistent_environment_with_root_and_resource_is_live() {
        let fixture = Fixture::new().await;
        let environment = fixture
            .environment(Persistence::Always, Some(dead_pid()))
            .await;
        assert_eq!(
            fixture.assess(&environment).await.status,
            EnvironmentStatus::Live
        );
    }

    #[tokio::test]
    async fn environment_without_host_root_is_stale() {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::Always, None).await;
        std::fs::remove_dir_all(&environment.root_dir).unwrap();
        assert_eq!(
            fixture.assess(&environment).await.status,
            EnvironmentStatus::Stale
        );
    }

    #[tokio::test]
    async fn still_stale_releases_the_claim_when_no_longer_stale() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::Always, None).await;

        assert!(
            !fixture
                .claim_and_destroy(&environment, DestroyWhen::StillStale)
                .await?
        );

        let row = fixture.row(&environment).await.unwrap();
        assert_eq!(row.owner, None);
        assert!(environment.root_dir.exists());
        // Released means no longer `destroying`, so Attachments are allowed.
        attach(&fixture.registry, &environment.id(), Process::current()).await?;
        Ok(())
    }

    #[tokio::test]
    async fn still_stale_destroys_a_stale_environment() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::No, Some(dead_pid())).await;

        assert!(
            fixture
                .claim_and_destroy(&environment, DestroyWhen::StillStale)
                .await?
        );

        assert!(fixture.row(&environment).await.is_none());
        assert!(!environment.root_dir.exists());
        Ok(())
    }

    #[tokio::test]
    async fn reachable_destroys_a_live_environment() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::Always, None).await;

        assert!(
            fixture
                .claim_and_destroy(&environment, DestroyWhen::Reachable)
                .await?
        );

        assert!(fixture.row(&environment).await.is_none());
        assert!(!environment.root_dir.exists());
        Ok(())
    }

    #[tokio::test]
    async fn claim_is_refused_while_an_attachment_is_live() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::Always, None).await;
        attach(&fixture.registry, &environment.id(), Process::current()).await?;

        let result = fixture
            .claim_and_destroy(&environment, DestroyWhen::Always)
            .await;

        assert!(result.is_err());
        assert!(fixture.row(&environment).await.is_some());
        assert!(environment.root_dir.exists());
        Ok(())
    }

    #[tokio::test]
    async fn assess_reports_live_attachment_count() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::Always, None).await;
        let assessed = fixture.assess(&environment).await;
        assert_eq!(assessed.status, EnvironmentStatus::Live);
        assert_eq!(assessed.attachments, 0);

        attach(&fixture.registry, &environment.id(), Process::current()).await?;
        assert_eq!(fixture.assess(&environment).await.attachments, 1);

        attach(
            &fixture.registry,
            &environment.id(),
            Process::of(dead_pid()),
        )
        .await?;
        assert_eq!(
            fixture.assess(&environment).await.attachments,
            1,
            "dead shells are reaped"
        );
        Ok(())
    }

    #[tokio::test]
    async fn shell_holds_and_releases_attachment() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::Always, None).await;
        let registered = fixture.row(&environment).await.unwrap();

        let mut attached = None;
        shell_while_attached(
            &fixture.registry,
            &DriverConfig::default(),
            &registered,
            |_driver_config, registered| {
                attached = Some(registered.environment.id());
                Ok(())
            },
        )
        .await?;
        assert_eq!(attached, Some(environment.id()));

        let assessed = fixture.assess(&environment).await;
        assert_eq!(assessed.status, EnvironmentStatus::Live);
        assert_eq!(assessed.attachments, 0);
        Ok(())
    }

    #[tokio::test]
    async fn shell_on_stale_environment_leaves_no_attachment() {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::Always, None).await;
        std::fs::remove_dir_all(&environment.root_dir).unwrap();
        let registered = fixture.row(&environment).await.unwrap();

        let error = shell_environment(&fixture.registry, &DriverConfig::default(), &registered)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Stale"));

        let assessed = fixture.assess(&environment).await;
        assert_eq!(assessed.status, EnvironmentStatus::Stale);
        assert_eq!(assessed.attachments, 0);
    }
}

#[cfg(test)]
mod claimed_environment_tests {
    use super::*;
    use crate::build_intent::BuildKind;
    use crate::driver::{DriverType, EnvironmentPurpose};
    use debmagic_common::distro::{Distro, DistroVersion};
    use std::path::{Path, PathBuf};

    struct Fixture {
        dir: PathBuf,
        registry: Registry,
        driver_config: DriverConfig,
        overrides: DriverOverrides,
    }

    impl Fixture {
        async fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("debmagic-lifecycle-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let registry = Registry::open(&dir.join("db.sqlite")).await.unwrap();
            Self {
                dir,
                registry,
                driver_config: DriverConfig::default(),
                overrides: DriverOverrides::default(),
            }
        }

        fn environment(&self, persistence: Persistence) -> Environment {
            Environment::new(
                DriverType::Bare,
                "pkg",
                "pkg-1.0",
                &self.dir.join("src"),
                DistroVersion::new(Distro::Debian, "trixie", "13"),
                persistence,
                EnvironmentPurpose::Build,
                &self.dir.join("environments"),
            )
        }

        async fn acquire(
            &self,
            environment: Environment,
            root_policy: HostRootPolicy,
            stage: impl FnOnce(&Environment, Option<&Driver>) -> anyhow::Result<()>,
        ) -> anyhow::Result<ClaimedEnvironment<'_>> {
            ClaimedEnvironment::acquire(
                &self.registry,
                environment,
                &self.driver_config,
                &AptConfig::default(),
                &self.overrides,
                root_policy,
                stage,
            )
            .await
        }

        async fn owner(&self, environment: &Environment) -> Option<Option<u32>> {
            self.registry
                .get(&environment.id())
                .await
                .unwrap()
                .map(|row| row.owner.map(|owner| owner.pid))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn write_staged_file(environment: &Environment) -> anyhow::Result<()> {
        std::fs::write(environment.staged_source_dir().join("staged"), "")?;
        Ok(())
    }

    fn staged_file(environment: &Environment) -> PathBuf {
        environment.staged_source_dir().join("staged")
    }

    #[tokio::test]
    async fn finish_tears_down_non_persistent_environment() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::No);
        let claimed = fixture
            .acquire(
                environment.clone(),
                HostRootPolicy::Reset,
                |environment, _| write_staged_file(environment),
            )
            .await?;
        assert!(staged_file(claimed.environment()).is_file());
        assert_eq!(
            fixture.owner(&environment).await,
            Some(Some(std::process::id()))
        );

        claimed.finish().await?;
        assert!(!environment.root_dir.exists());
        assert_eq!(fixture.owner(&environment).await, None);
        Ok(())
    }

    #[tokio::test]
    async fn staging_failure_releases_claim_and_host_root() {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::No);
        let result = fixture
            .acquire(
                environment.clone(),
                HostRootPolicy::Reset,
                |environment, _| {
                    write_staged_file(environment)?;
                    anyhow::bail!("staging failed")
                },
            )
            .await;
        assert!(result.is_err());
        assert!(!environment.root_dir.exists());
        assert_eq!(fixture.owner(&environment).await, None);
    }

    #[tokio::test]
    async fn persistent_environment_stays_registered_without_owner() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::Always);
        fixture
            .acquire(
                environment.clone(),
                HostRootPolicy::Reset,
                |environment, _| write_staged_file(environment),
            )
            .await?
            .finish()
            .await?;
        assert!(staged_file(&environment).is_file());
        assert_eq!(fixture.owner(&environment).await, Some(None));
        Ok(())
    }

    #[tokio::test]
    async fn reused_host_root_is_kept_only_for_incremental_with_manifest() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::Always);
        let manifest = environment.root_dir.join("manifest");
        let keep = || HostRootPolicy::KeepIncremental {
            manifest: manifest.clone(),
        };
        let no_stage = |_: &Environment, _: Option<&Driver>| Ok(());

        fixture
            .acquire(environment.clone(), keep(), |environment, _| {
                write_staged_file(environment)
            })
            .await?
            .finish()
            .await?;
        fixture
            .acquire(environment.clone(), keep(), no_stage)
            .await?
            .finish()
            .await?;
        assert!(!staged_file(&environment).exists(), "no manifest: reset");

        fixture
            .acquire(environment.clone(), keep(), |environment, _| {
                write_staged_file(environment)
            })
            .await?
            .finish()
            .await?;
        std::fs::write(&manifest, "")?;
        fixture
            .acquire(environment.clone(), keep(), no_stage)
            .await?
            .finish()
            .await?;
        assert!(staged_file(&environment).is_file(), "manifest: kept");

        fixture
            .acquire(environment.clone(), HostRootPolicy::Reset, no_stage)
            .await?
            .finish()
            .await?;
        assert!(
            !staged_file(&environment).exists(),
            "Reset ignores manifest"
        );
        assert!(Path::new(&environment.staged_source_dir()).is_dir());
        Ok(())
    }

    #[tokio::test]
    async fn staging_failure_on_reused_root_releases_claim() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::Always);
        fixture
            .acquire(
                environment.clone(),
                HostRootPolicy::Reset,
                |environment, _| write_staged_file(environment),
            )
            .await?
            .finish()
            .await?;
        let result = fixture
            .acquire(environment.clone(), HostRootPolicy::Reset, |_, _| {
                anyhow::bail!("staging failed")
            })
            .await;
        assert!(result.is_err());
        assert_eq!(fixture.owner(&environment).await, Some(None));
        Ok(())
    }

    const SOURCE_BUILD: InvocationKind = InvocationKind::Build(BuildKind::Source);

    async fn claim(
        fixture: &Fixture,
        environment: Environment,
        command: impl FnOnce(&ClaimedEnvironment<'_>) -> anyhow::Result<CommandCompletion<()>>,
    ) -> anyhow::Result<()> {
        claim_command(
            &fixture.registry,
            environment,
            &fixture.driver_config,
            &AptConfig::default(),
            &fixture.overrides,
            HostRootPolicy::Reset,
            |_, _| Ok(()),
            SOURCE_BUILD,
            "9.9",
            command,
        )
        .await
    }

    fn completion(value: anyhow::Result<()>) -> CommandCompletion<()> {
        let success = value.is_ok();
        CommandCompletion {
            value,
            success,
            shell_worthy: !success,
            finish_error_fails_command: success,
            changes_path: None,
            failure_lines: Vec::new(),
        }
    }

    #[tokio::test]
    async fn claim_command_records_a_successful_completion() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::No);
        claim(&fixture, environment.clone(), |_| Ok(completion(Ok(())))).await?;
        let rows = fixture
            .registry
            .invocations_for_source(&environment.source_dir)
            .await?;
        assert_eq!(rows.len(), 1);
        assert!(rows[0].success);
        assert_eq!(rows[0].kind, SOURCE_BUILD);
        assert_eq!(rows[0].package_version, "9.9");
        assert_eq!(rows[0].driver, Some(DriverType::Bare));
        assert_eq!(rows[0].source_dir, environment.source_dir);
        // the Environment row is gone, the Invocation stays
        assert_eq!(rows[0].environment_id, None);
        assert!(!environment.root_dir.exists());
        Ok(())
    }

    #[tokio::test]
    async fn claim_command_records_a_failed_completion() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::No);
        let error = claim(&fixture, environment.clone(), |_| {
            Ok(completion(Err(anyhow::anyhow!("build failed"))))
        })
        .await
        .unwrap_err();
        assert!(error.to_string().contains("build failed"));
        let rows = fixture
            .registry
            .invocations_for_source(&environment.source_dir)
            .await?;
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].success);
        assert_eq!(rows[0].package_version, "9.9");
        assert!(!environment.root_dir.exists());
        Ok(())
    }

    #[tokio::test]
    async fn claim_command_tears_down_and_records_nothing_when_the_command_stops()
    -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::No);
        let error = claim(&fixture, environment.clone(), |_| anyhow::bail!("stopped"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("stopped"));
        let rows = fixture
            .registry
            .invocations_for_source(&environment.source_dir)
            .await?;
        assert!(rows.is_empty());
        assert!(!environment.root_dir.exists());
        assert_eq!(fixture.owner(&environment).await, None);
        Ok(())
    }

    #[tokio::test]
    async fn on_failure_success_tears_the_environment_down() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::OnFailure);
        claim(&fixture, environment.clone(), |_| Ok(completion(Ok(())))).await?;
        assert!(!environment.root_dir.exists());
        assert_eq!(fixture.owner(&environment).await, None);
        Ok(())
    }

    #[tokio::test]
    async fn on_failure_command_failure_keeps_the_environment() -> anyhow::Result<()> {
        let fixture = Fixture::new().await;
        let environment = fixture.environment(Persistence::OnFailure);
        let error = claim(&fixture, environment.clone(), |_| {
            Ok(completion(Err(anyhow::anyhow!("build failed"))))
        })
        .await
        .unwrap_err();
        assert!(error.to_string().contains("build failed"));
        assert!(environment.root_dir.exists());
        assert_eq!(fixture.owner(&environment).await, Some(None));
        Ok(())
    }

    #[test]
    fn inspection_hint_names_the_environment() {
        assert_eq!(
            inspection_hint("bld-abc"),
            "Get a shell with `debmagic env shell bld-abc`"
        );
    }
}
