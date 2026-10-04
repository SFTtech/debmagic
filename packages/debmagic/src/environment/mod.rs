mod registry;

pub use registry::{
    InvocationKind, RecordedInvocation, RegisteredEnvironment, Registry, prior_binary_invocation,
    select_unique_environment,
};

use std::path::PathBuf;

use anyhow::{Context, bail};
use registry::current_pid;

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
/// its Host root prepared and its Driver created. Tear it down with
/// [`Self::finish`]; dropping it unfinished tears it down best-effort.
pub struct ClaimedEnvironment<'a> {
    registry: &'a Registry,
    driver_config: &'a DriverConfig,
    environment: Environment,
    driver: Driver,
    /// `on-failure` keeps the Environment only when the build or TestRun
    /// command itself failed. Set before [`Self::finish`].
    keep_after_failure: bool,
    finished: bool,
}

impl<'a> ClaimedEnvironment<'a> {
    /// Claim `environment`, prepare its Host root, run `stage` to fill it
    /// (after the Environment directories exist), and create its Driver.
    ///
    /// A fresh Host root is staged before the Driver is created, because
    /// container Drivers bind-mount it. A reused one gets its Driver first,
    /// so the Driver can reset the root from inside the Environment.
    pub fn acquire(
        registry: &'a Registry,
        environment: Environment,
        driver_config: &'a DriverConfig,
        overrides: &DriverOverrides,
        root_policy: HostRootPolicy,
        stage: impl FnOnce(&Environment, Option<&Driver>) -> anyhow::Result<()>,
    ) -> anyhow::Result<Self> {
        let planned = planned_driver_metadata(&environment, driver_config, overrides);
        let previous = registry
            .begin_environment(&environment, current_pid(), &planned)?
            .map(|row| row.metadata());

        if environment.persistence.reuses() && environment.root_dir.exists() {
            let claimed = Self::create_driver(registry, environment, driver_config, overrides)?;
            claimed.prepare_reused_root(root_policy, stage)?;
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
            abandon_claim(registry, &environment, driver_config, false);
            return Err(error);
        }
        Self::create_driver(registry, environment, driver_config, overrides)
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn driver(&self) -> &Driver {
        &self.driver
    }

    pub fn finish(mut self) -> anyhow::Result<()> {
        self.finished = true;
        finish_environment(
            self.registry,
            &self.environment,
            &self.driver,
            self.driver_config,
            self.keep_after_failure,
        )
    }

    fn create_driver(
        registry: &'a Registry,
        environment: Environment,
        driver_config: &'a DriverConfig,
        overrides: &DriverOverrides,
    ) -> anyhow::Result<Self> {
        let driver = match crate::driver::create_driver(&environment, driver_config, overrides) {
            Ok(driver) => driver,
            Err(error) => {
                abandon_claim(registry, &environment, driver_config, true);
                return Err(
                    error.context(format!("failed to create {:?} driver", environment.driver))
                );
            }
        };
        // Best effort: the claim already recorded the planned Driver metadata,
        // which is enough to find the resource.
        if let Err(error) =
            registry.set_driver_metadata(&environment.id(), &driver.driver_metadata())
        {
            eprintln!("Warning: failed to record Driver metadata: {error}");
        }
        Ok(Self {
            registry,
            driver_config,
            environment,
            driver,
            keep_after_failure: false,
            finished: false,
        })
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

impl Drop for ClaimedEnvironment<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        if let Err(error) = finish_environment(
            self.registry,
            &self.environment,
            &self.driver,
            self.driver_config,
            self.keep_after_failure,
        ) {
            eprintln!(
                "Warning: failed to tear down Environment {}: {error:#}",
                self.environment.id()
            );
        }
    }
}

/// What a command finished with, once an Environment is claimed.
///
/// `Err` from the command callback is not a completion: nothing is recorded,
/// and dropping the claim tears the Environment down with a warning.
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
pub fn claim_command<T>(
    registry: &Registry,
    environment: Environment,
    driver_config: &DriverConfig,
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
        overrides,
        root_policy,
        stage,
    )?;
    let completion = command(&claimed)?;

    let source_dir = claimed.environment().source_dir.clone();
    let package_name = claimed.environment().package_name.clone();
    let distro = claimed.environment().distro.clone();
    let driver = claimed.environment().driver;
    let environment_id = claimed.environment().id();
    let purpose = claimed.environment().purpose.as_str();
    if let Err(error) = registry.record_invocation(
        kind,
        &source_dir,
        &package_name,
        package_version,
        &distro,
        Some(driver),
        completion.success,
        completion.changes_path.as_deref(),
        Some(&environment_id),
    ) {
        eprintln!("Warning: failed to record Invocation: {error}");
    }

    if completion.shell_worthy {
        for line in &completion.failure_lines {
            eprintln!("{line}");
        }
        if claimed.environment().persistence.reuses() {
            eprintln!("{}", inspection_hint(&environment_id));
            claimed.keep_after_failure = true;
        }
    }

    let finish_error_fails = completion.finish_error_fails_command;
    let value = completion.value;
    match claimed.finish() {
        Ok(()) => value,
        Err(error) if finish_error_fails => {
            Err(error.context(format!("failed to clean up {purpose} environment")))
        }
        Err(error) => {
            eprintln!("Failed to clean up {purpose} environment: {error}");
            value
        }
    }
}

/// Undo a claim when acquiring failed before a Driver existed. A failed
/// Driver creation may have left a resource under the planned metadata, so
/// then the row stays (unowned, hence Stale) for `debmagic env clean`.
fn abandon_claim(
    registry: &Registry,
    environment: &Environment,
    driver_config: &DriverConfig,
    driver_attempted: bool,
) {
    let id = environment.id();
    let owner = current_pid();
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
        registry.finish_destroy(&id, owner)
    } else {
        registry.clear_owner(&id, owner)
    };
    if let Err(error) = released {
        eprintln!("Warning: failed to release Environment {id} in the registry: {error:#}");
    }
}

fn finish_environment(
    registry: &Registry,
    environment: &Environment,
    driver: &Driver,
    driver_config: &DriverConfig,
    keep_after_failure: bool,
) -> anyhow::Result<()> {
    let id = environment.id();
    let owner = current_pid();
    let keep = match environment.persistence {
        Persistence::Always => true,
        Persistence::OnFailure => keep_after_failure,
        Persistence::No => false,
    };
    if keep {
        registry.wait_until_no_attachments(&id)?;
        registry.clear_owner(&id, owner)?;
        return Ok(());
    }

    // Keep ownership while destroying: concurrent `begin_environment` calls
    // see a live owner and refuse, and the atomic wait-and-mark blocks new
    // Attachments, so nothing can start using the Environment mid-teardown.
    registry.wait_and_mark_destroying(&id, owner)?;
    let metadata = EnvironmentMetadata {
        environment: environment.clone(),
        driver_metadata: driver.driver_metadata(),
    };
    destroy_claimed(registry, &metadata, Some(driver), driver_config, false)
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
pub fn assess_environment(
    registry: &Registry,
    registered: &RegisteredEnvironment,
    driver_config: &DriverConfig,
) -> anyhow::Result<AssessedEnvironment> {
    // Fail closed: a registry error must never look like "no Attachments",
    // since this status drives destruction.
    let attachments = registry.live_attachment_count(&registered.environment.id())?;
    let leftover = registered.environment.persistence == Persistence::No
        && Registry::owner_is_gone(registered.owner_pid, registered.owner_pid_start)
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
pub fn shell_environment(
    registry: &Registry,
    driver_config: &DriverConfig,
    registered: &RegisteredEnvironment,
) -> anyhow::Result<()> {
    shell_while_attached(registry, driver_config, registered, open_interactive_shell)
}

/// Assess, hold an Attachment around `during`, then release it. `during` is
/// the interactive shell in production. Tests pass their own check so they
/// exercise this same path without spawning `$SHELL` on the terminal.
fn shell_while_attached(
    registry: &Registry,
    driver_config: &DriverConfig,
    registered: &RegisteredEnvironment,
    during: impl FnOnce(&DriverConfig, &RegisteredEnvironment) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let id = registered.environment.id();
    match assess_environment(registry, registered, driver_config)?.status {
        EnvironmentStatus::Unreachable => bail!("Environment {id} is Unreachable"),
        EnvironmentStatus::Stale => {
            bail!("Environment {id} is Stale; run `debmagic env clean {id}`")
        }
        EnvironmentStatus::Live => {}
    }

    let attachment_id = registry.add_attachment(&id, current_pid())?;
    let result = during(driver_config, registered);
    let removal = registry.remove_attachment(attachment_id);
    result.and(removal)
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
    /// `env clean` without id: only what is still Stale once claimed.
    StillStale,
    /// `env clean <id>`: anything but Unreachable.
    Reachable,
    /// `env clean <id> --force`: also without Driver cooperation; Driver
    /// failures only warn.
    Always,
}

/// Claim the Environment for destruction (refusing live Attachments and a
/// live owner), then re-assess it from the current row: between the
/// caller's assessment and the claim, a run may have recreated it. Destroys
/// the Driver resource, Host root, and registry row when `when` still holds;
/// otherwise releases the claim and returns `false`.
pub fn claim_and_destroy(
    registry: &Registry,
    driver_config: &DriverConfig,
    id: &str,
    when: DestroyWhen,
) -> anyhow::Result<bool> {
    let owner = current_pid();
    registry.claim_for_destroy(id)?;
    let assessed = (|| -> anyhow::Result<Option<RegisteredEnvironment>> {
        let mut claimed = registry
            .get(id)?
            .with_context(|| format!("no Environment with id {id}"))?;
        // The claim verified the previous owner was gone; we are only
        // holding the row, which must not make it look in use.
        claimed.owner_pid = None;
        claimed.owner_pid_start = None;
        let status = assess_environment(registry, &claimed, driver_config)?.status;
        let proceed = match when {
            DestroyWhen::StillStale => status == EnvironmentStatus::Stale,
            DestroyWhen::Reachable => status != EnvironmentStatus::Unreachable,
            DestroyWhen::Always => true,
        };
        Ok(proceed.then_some(claimed))
    })();
    let claimed = match assessed {
        Ok(Some(claimed)) => claimed,
        Ok(None) => {
            registry.release_destroy_claim(id, owner)?;
            return Ok(false);
        }
        Err(error) => {
            let _ = registry.release_destroy_claim(id, owner);
            return Err(error);
        }
    };

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
    destroy_claimed(registry, &metadata, driver.as_ref(), driver_config, force)?;
    Ok(true)
}

/// Destroy an Environment this process has marked `destroying`: its Driver
/// resource, Host root, and registry row, in that order. Any failure keeps
/// the claim, so the row stays `destroying` (no new Attachments) and still
/// records the resource for a later `env clean`. With `force`, a failed
/// resource destroy only warns.
fn destroy_claimed(
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
    registry.finish_destroy(&id, current_pid())?;
    Ok(())
}

#[cfg(test)]
mod teardown_tests {
    use std::path::PathBuf;

    use debmagic_common::distro::{Distro, DistroVersion};

    use super::*;
    use crate::driver::{DriverType, EnvironmentPurpose};

    struct Fixture {
        dir: PathBuf,
        registry: Registry,
    }

    impl Fixture {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("debmagic-lifecycle-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let registry = Registry::open(&dir.join("db.sqlite")).unwrap();
            Self { dir, registry }
        }

        /// A registered Bare-driver Environment whose Host root exists.
        fn environment(&self, persistence: Persistence, owner_pid: Option<u32>) -> Environment {
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
            self.registry
                .upsert_environment(&environment, owner_pid)
                .unwrap();
            environment
        }

        fn row(&self, environment: &Environment) -> Option<RegisteredEnvironment> {
            self.registry.get(&environment.id()).unwrap()
        }

        fn assess(&self, environment: &Environment) -> EnvironmentStatus {
            let registered = self.row(environment).unwrap();
            assess_environment(&self.registry, &registered, &DriverConfig::default())
                .unwrap()
                .status
        }

        fn claim_and_destroy(&self, environment: &Environment, when: DestroyWhen) -> bool {
            claim_and_destroy(
                &self.registry,
                &DriverConfig::default(),
                &environment.id(),
                when,
            )
            .unwrap()
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

    #[test]
    fn non_persistent_environment_with_dead_owner_is_stale() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::No, Some(dead_pid()));
        assert_eq!(fixture.assess(&environment), EnvironmentStatus::Stale);
    }

    #[test]
    fn non_persistent_environment_with_live_owner_is_live() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::No, Some(current_pid()));
        assert_eq!(fixture.assess(&environment), EnvironmentStatus::Live);
    }

    #[test]
    fn persistent_environment_with_root_and_resource_is_live() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::Always, Some(dead_pid()));
        assert_eq!(fixture.assess(&environment), EnvironmentStatus::Live);
    }

    #[test]
    fn environment_without_host_root_is_stale() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::Always, None);
        std::fs::remove_dir_all(&environment.root_dir).unwrap();
        assert_eq!(fixture.assess(&environment), EnvironmentStatus::Stale);
    }

    #[test]
    fn still_stale_releases_the_claim_when_no_longer_stale() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::Always, None);

        assert!(!fixture.claim_and_destroy(&environment, DestroyWhen::StillStale));

        let row = fixture.row(&environment).unwrap();
        assert_eq!(row.owner_pid, None);
        assert!(environment.root_dir.exists());
        // Released means no longer `destroying`, so Attachments are allowed.
        fixture
            .registry
            .add_attachment(&environment.id(), current_pid())
            .unwrap();
    }

    #[test]
    fn still_stale_destroys_a_stale_environment() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::No, Some(dead_pid()));

        assert!(fixture.claim_and_destroy(&environment, DestroyWhen::StillStale));

        assert!(fixture.row(&environment).is_none());
        assert!(!environment.root_dir.exists());
    }

    #[test]
    fn reachable_destroys_a_live_environment() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::Always, None);

        assert!(fixture.claim_and_destroy(&environment, DestroyWhen::Reachable));

        assert!(fixture.row(&environment).is_none());
        assert!(!environment.root_dir.exists());
    }

    #[test]
    fn claim_is_refused_while_an_attachment_is_live() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::Always, None);
        fixture
            .registry
            .add_attachment(&environment.id(), current_pid())
            .unwrap();

        let result = claim_and_destroy(
            &fixture.registry,
            &DriverConfig::default(),
            &environment.id(),
            DestroyWhen::Always,
        );

        assert!(result.is_err());
        assert!(fixture.row(&environment).is_some());
        assert!(environment.root_dir.exists());
    }

    #[test]
    fn assess_reports_live_attachment_count() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::Always, None);
        let registered = fixture.row(&environment).unwrap();
        let assessed =
            assess_environment(&fixture.registry, &registered, &DriverConfig::default()).unwrap();
        assert_eq!(assessed.status, EnvironmentStatus::Live);
        assert_eq!(assessed.attachments, 0);

        fixture
            .registry
            .add_attachment(&environment.id(), current_pid())
            .unwrap();
        let assessed = assess_environment(
            &fixture.registry,
            &fixture.row(&environment).unwrap(),
            &DriverConfig::default(),
        )
        .unwrap();
        assert_eq!(assessed.attachments, 1);
    }

    #[test]
    fn shell_holds_and_releases_attachment() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::Always, None);
        let registered = fixture.row(&environment).unwrap();

        let registry = &fixture.registry;
        shell_while_attached(
            registry,
            &DriverConfig::default(),
            &registered,
            |_driver_config, registered| {
                assert_eq!(
                    registry
                        .live_attachment_count(&registered.environment.id())
                        .unwrap(),
                    1
                );
                Ok(())
            },
        )
        .unwrap();

        let assessed = assess_environment(
            &fixture.registry,
            &fixture.row(&environment).unwrap(),
            &DriverConfig::default(),
        )
        .unwrap();
        assert_eq!(assessed.status, EnvironmentStatus::Live);
        assert_eq!(assessed.attachments, 0);
    }

    #[test]
    fn shell_on_stale_environment_leaves_no_attachment() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::Always, None);
        std::fs::remove_dir_all(&environment.root_dir).unwrap();
        let registered = fixture.row(&environment).unwrap();

        let error = shell_environment(&fixture.registry, &DriverConfig::default(), &registered)
            .unwrap_err();
        assert!(error.to_string().contains("Stale"));

        let assessed =
            assess_environment(&fixture.registry, &registered, &DriverConfig::default()).unwrap();
        assert_eq!(assessed.status, EnvironmentStatus::Stale);
        assert_eq!(assessed.attachments, 0);
    }
}

#[cfg(test)]
mod claimed_environment_tests {
    use super::*;
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
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("debmagic-lifecycle-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let registry = Registry::open(&dir.join("db.sqlite")).unwrap();
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

        fn acquire(
            &self,
            environment: Environment,
            root_policy: HostRootPolicy,
            stage: impl FnOnce(&Environment, Option<&Driver>) -> anyhow::Result<()>,
        ) -> anyhow::Result<ClaimedEnvironment<'_>> {
            ClaimedEnvironment::acquire(
                &self.registry,
                environment,
                &self.driver_config,
                &self.overrides,
                root_policy,
                stage,
            )
        }

        fn owner(&self, environment: &Environment) -> Option<Option<u32>> {
            self.registry
                .get(&environment.id())
                .unwrap()
                .map(|row| row.owner_pid)
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

    #[test]
    fn finish_tears_down_non_persistent_environment() -> anyhow::Result<()> {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::No);
        let claimed = fixture.acquire(
            environment.clone(),
            HostRootPolicy::Reset,
            |environment, _| write_staged_file(environment),
        )?;
        assert!(staged_file(claimed.environment()).is_file());
        assert_eq!(fixture.owner(&environment), Some(Some(current_pid())));

        claimed.finish()?;
        assert!(!environment.root_dir.exists());
        assert_eq!(fixture.owner(&environment), None);
        Ok(())
    }

    #[test]
    fn dropping_unfinished_tears_down() -> anyhow::Result<()> {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::No);
        drop(fixture.acquire(
            environment.clone(),
            HostRootPolicy::Reset,
            |environment, _| write_staged_file(environment),
        )?);
        assert!(!environment.root_dir.exists());
        assert_eq!(fixture.owner(&environment), None);
        Ok(())
    }

    #[test]
    fn staging_failure_releases_claim_and_host_root() {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::No);
        let result = fixture.acquire(
            environment.clone(),
            HostRootPolicy::Reset,
            |environment, _| {
                write_staged_file(environment)?;
                anyhow::bail!("staging failed")
            },
        );
        assert!(result.is_err());
        assert!(!environment.root_dir.exists());
        assert_eq!(fixture.owner(&environment), None);
    }

    #[test]
    fn persistent_environment_stays_registered_without_owner() -> anyhow::Result<()> {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::Always);
        fixture
            .acquire(
                environment.clone(),
                HostRootPolicy::Reset,
                |environment, _| write_staged_file(environment),
            )?
            .finish()?;
        assert!(staged_file(&environment).is_file());
        assert_eq!(fixture.owner(&environment), Some(None));
        Ok(())
    }

    #[test]
    fn reused_host_root_is_kept_only_for_incremental_with_manifest() -> anyhow::Result<()> {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::Always);
        let manifest = environment.root_dir.join("manifest");
        let keep = || HostRootPolicy::KeepIncremental {
            manifest: manifest.clone(),
        };
        let no_stage = |_: &Environment, _: Option<&Driver>| Ok(());

        fixture
            .acquire(environment.clone(), keep(), |environment, _| {
                write_staged_file(environment)
            })?
            .finish()?;
        fixture
            .acquire(environment.clone(), keep(), no_stage)?
            .finish()?;
        assert!(!staged_file(&environment).exists(), "no manifest: reset");

        fixture
            .acquire(environment.clone(), keep(), |environment, _| {
                write_staged_file(environment)
            })?
            .finish()?;
        std::fs::write(&manifest, "")?;
        fixture
            .acquire(environment.clone(), keep(), no_stage)?
            .finish()?;
        assert!(staged_file(&environment).is_file(), "manifest: kept");

        fixture
            .acquire(environment.clone(), HostRootPolicy::Reset, no_stage)?
            .finish()?;
        assert!(
            !staged_file(&environment).exists(),
            "Reset ignores manifest"
        );
        assert!(Path::new(&environment.staged_source_dir()).is_dir());
        Ok(())
    }

    #[test]
    fn staging_failure_on_reused_root_releases_claim() -> anyhow::Result<()> {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::Always);
        fixture
            .acquire(
                environment.clone(),
                HostRootPolicy::Reset,
                |environment, _| write_staged_file(environment),
            )?
            .finish()?;
        let result = fixture.acquire(environment.clone(), HostRootPolicy::Reset, |_, _| {
            anyhow::bail!("staging failed")
        });
        assert!(result.is_err());
        assert_eq!(fixture.owner(&environment), Some(None));
        Ok(())
    }

    fn claim(
        fixture: &Fixture,
        environment: Environment,
        command: impl FnOnce(&ClaimedEnvironment<'_>) -> anyhow::Result<CommandCompletion<()>>,
    ) -> anyhow::Result<()> {
        claim_command(
            &fixture.registry,
            environment,
            &fixture.driver_config,
            &fixture.overrides,
            HostRootPolicy::Reset,
            |_, _| Ok(()),
            InvocationKind::SourceBuild,
            "9.9",
            command,
        )
    }

    #[test]
    fn claim_command_records_a_successful_completion() -> anyhow::Result<()> {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::No);
        claim(&fixture, environment.clone(), |_| {
            Ok(CommandCompletion {
                value: Ok(()),
                success: true,
                shell_worthy: false,
                finish_error_fails_command: true,
                changes_path: None,
                failure_lines: Vec::new(),
            })
        })?;
        let rows = fixture
            .registry
            .invocations_for_source(&environment.source_dir)?;
        assert_eq!(rows.len(), 1);
        assert!(rows[0].success);
        assert_eq!(rows[0].kind, InvocationKind::SourceBuild);
        assert_eq!(rows[0].package_version, "9.9");
        assert_eq!(rows[0].driver, Some(DriverType::Bare));
        assert_eq!(rows[0].source_dir, environment.source_dir);
        assert!(!environment.root_dir.exists());
        Ok(())
    }

    #[test]
    fn claim_command_records_a_failed_completion() -> anyhow::Result<()> {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::No);
        let error = claim(&fixture, environment.clone(), |_| {
            Ok(CommandCompletion {
                value: Err(anyhow::anyhow!("build failed")),
                success: false,
                shell_worthy: true,
                finish_error_fails_command: false,
                changes_path: None,
                failure_lines: vec!["Build failed: build failed".into()],
            })
        })
        .unwrap_err();
        assert!(error.to_string().contains("build failed"));
        let rows = fixture
            .registry
            .invocations_for_source(&environment.source_dir)?;
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].success);
        assert_eq!(rows[0].package_version, "9.9");
        assert!(!environment.root_dir.exists());
        Ok(())
    }

    #[test]
    fn claim_command_records_nothing_when_the_command_stops() -> anyhow::Result<()> {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::No);
        let error = claim(&fixture, environment.clone(), |_| anyhow::bail!("stopped")).unwrap_err();
        assert!(error.to_string().contains("stopped"));
        let rows = fixture
            .registry
            .invocations_for_source(&environment.source_dir)?;
        assert!(rows.is_empty());
        assert!(!environment.root_dir.exists());
        Ok(())
    }

    #[test]
    fn on_failure_success_tears_the_environment_down() -> anyhow::Result<()> {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::OnFailure);
        claim(&fixture, environment.clone(), |_| {
            Ok(CommandCompletion {
                value: Ok(()),
                success: true,
                shell_worthy: false,
                finish_error_fails_command: true,
                changes_path: None,
                failure_lines: Vec::new(),
            })
        })?;
        assert!(!environment.root_dir.exists());
        assert_eq!(fixture.owner(&environment), None);
        Ok(())
    }

    #[test]
    fn on_failure_command_failure_keeps_the_environment() -> anyhow::Result<()> {
        let fixture = Fixture::new();
        let environment = fixture.environment(Persistence::OnFailure);
        let error = claim(&fixture, environment.clone(), |_| {
            Ok(CommandCompletion {
                value: Err(anyhow::anyhow!("build failed")),
                success: false,
                shell_worthy: true,
                finish_error_fails_command: false,
                changes_path: None,
                failure_lines: vec!["Build failed: build failed".into()],
            })
        })
        .unwrap_err();
        assert!(error.to_string().contains("build failed"));
        assert!(environment.root_dir.exists());
        assert_eq!(fixture.owner(&environment), Some(None));
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
