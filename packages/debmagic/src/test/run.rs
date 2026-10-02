use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::intent::TestIntent;
use crate::build::artifacts::{copy_changes_artifacts, copy_dir_all};
use crate::build::source::{stage_dir, stage_source_tree};
use crate::build_intent::BuildKind;
use crate::driver::{DriverType, Environment, EnvironmentDriver, EnvironmentPurpose};
use crate::driver::{IsolationCapability, config::DriverConfig};
use crate::environment::{
    CommandCompletion, HostRootPolicy, InvocationKind, RecordedInvocation, Registry, claim_command,
    prior_binary_invocation,
};
use crate::package::distro_resolve_mode_for_driver;
use crate::package::load_package;
use crate::subprocess::Capture;
use anyhow::{Context, anyhow, bail};
use debian_control::lossless::changes::Changes;
use debmagic_common::distro::DistroVersion;
use debmagic_common::package::SourcePackage;

/// autopkgtest(1) exit status values (Debian autopkgtest 6.x).
/// Some codes combine categories (e.g. 6 = 4|2); treat them as bitmasks where noted.
pub const AUTOPKGTEST_EXIT_PASS: i32 = 0;
pub const AUTOPKGTEST_EXIT_SKIP: i32 = 2;
pub const AUTOPKGTEST_EXIT_FAIL: i32 = 4;
pub const AUTOPKGTEST_EXIT_NO_TESTS: i32 = 8;
pub const AUTOPKGTEST_EXIT_ERRONEOUS_PKG: i32 = 12;
pub const AUTOPKGTEST_EXIT_TESTBED_FAILURE: i32 = 16;
pub const AUTOPKGTEST_EXIT_OTHER: i32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestOutcome {
    Passed,
    Failed,
    StrictFailure,
}

fn map_autopkgtest_exit(exit_code: i32, strict: bool) -> TestOutcome {
    if exit_code == AUTOPKGTEST_EXIT_PASS {
        return TestOutcome::Passed;
    }
    if exit_code < 0 {
        return TestOutcome::Failed;
    }

    let has_fail = (exit_code & AUTOPKGTEST_EXIT_FAIL) != 0
        || exit_code == AUTOPKGTEST_EXIT_TESTBED_FAILURE
        || exit_code == AUTOPKGTEST_EXIT_OTHER
        || exit_code == AUTOPKGTEST_EXIT_ERRONEOUS_PKG;
    if has_fail {
        return TestOutcome::Failed;
    }

    let has_skip = (exit_code & AUTOPKGTEST_EXIT_SKIP) != 0;
    let has_no_tests = (exit_code & AUTOPKGTEST_EXIT_NO_TESTS) != 0;
    if strict && (has_skip || has_no_tests) {
        return TestOutcome::StrictFailure;
    }

    TestOutcome::Passed
}

/// Resolve an explicit `--distro` override like a build would: built-in
/// suites, plus custom suites declared in the chosen Driver's `base_images`.
fn lookup_distro_override(
    name: &str,
    driver: DriverType,
    driver_config: &DriverConfig,
) -> anyhow::Result<DistroVersion> {
    let mode = distro_resolve_mode_for_driver(
        driver,
        &driver_config.docker.base_images,
        &driver_config.lxd.base_images,
    );
    crate::package::lookup_distro(name, mode)
}

/// Whether any suite in a `.changes` `Distribution` field (space-separated,
/// codenames or aliases like `unstable`) names `distro`.
fn distribution_matches(distribution: &str, distro: &DistroVersion) -> bool {
    distribution.split_whitespace().any(|suite| {
        suite == distro.codename
            || debmagic_common::distro::get_distro_version(suite)
                .is_some_and(|resolved| resolved.codename == distro.codename)
    })
}

/// Point out where the prior binary-build Invocation that supplies the
/// defaults disagrees with what is about to be tested.
fn warn_on_prior_build_mismatch(
    intent: &TestIntent,
    package: &SourcePackage,
    invocation: &RecordedInvocation,
    changes_path: &Path,
) {
    let current_version = package.version().to_string();
    if intent.changes.is_none() && invocation.package_version != current_version {
        eprintln!(
            "Warning: testing {} {} from the last successful binary build; \
             debian/changelog is now at {current_version}",
            package.name(),
            invocation.package_version
        );
    }
    if intent.changes.is_some()
        && intent.distro.is_none()
        && let Ok(changes) = Changes::from_file(changes_path)
        && let Some(distribution) = changes.distribution()
        && !distribution_matches(&distribution, &invocation.distro)
    {
        eprintln!(
            "Warning: {} targets {distribution}, but the test defaults to {} from the \
             prior binary build; pass --distro to override",
            changes_path.display(),
            invocation.distro.codename
        );
    }
}

/// Written into every exported test directory; only directories carrying it
/// are ever replaced, so an unrelated folder next to the `.changes` survives.
const TEST_OUTPUT_MARKER: &str = ".debmagic-test-output";

/// `<dir>/<changes stem>.test` — named after the `.changes` so it cannot
/// clash with a generic folder (e.g. a package's own `test/`) when the
/// output directory is the source tree or its parent.
fn exported_test_dir(changes_path: &Path) -> anyhow::Result<PathBuf> {
    let stem = changes_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| anyhow!("invalid .changes path: {}", changes_path.display()))?;
    let parent = changes_path.parent().unwrap_or_else(|| Path::new("."));
    Ok(parent.join(format!("{stem}.test")))
}

fn ensure_exported_test_dir_replaceable(dir: &Path) -> anyhow::Result<()> {
    if dir.exists() && !dir.join(TEST_OUTPUT_MARKER).is_file() {
        bail!(
            "{} exists but was not written by debmagic; move it away and retry",
            dir.display()
        );
    }
    Ok(())
}

fn replace_exported_test_dir(dir: &Path) -> anyhow::Result<()> {
    ensure_exported_test_dir_replaceable(dir)?;
    if !dir.exists() {
        return Ok(());
    }
    fs::remove_dir_all(dir).with_context(|| format!("failed to remove {}", dir.display()))
}

fn print_autopkgtest_notices(exit_code: i32, summary_path: &Path) {
    match fs::read_to_string(summary_path) {
        Ok(summary) if !summary.trim().is_empty() => {
            eprintln!("autopkgtest summary:");
            for line in summary.lines() {
                eprintln!("  {line}");
            }
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            eprintln!(
                "failed to read autopkgtest summary from {}: {error}",
                summary_path.display()
            );
        }
    }

    if (exit_code & AUTOPKGTEST_EXIT_NO_TESTS) != 0 || exit_code == AUTOPKGTEST_EXIT_NO_TESTS {
        eprintln!(
            "WARNING: autopkgtest reported no tests declared in this package (exit code {exit_code})"
        );
    }
}

/// autopkgtest(1) `--ignore-restrictions` for this IsolationCapability and
/// every rung below it.
///
/// virt-null cannot advertise isolation via `--fake-capability`: it always
/// sets a host downtmp prefix, and autopkgtest 5.49+ asserts that
/// isolation-container/machine and downtmp-host are mutually exclusive
/// (testbed failure, exit 16). Ignoring only the rungs this Environment
/// actually provides is the same honesty rule: isolation-container tests
/// run on Docker/LXD/Incus and still skip on Bare; isolation-machine still
/// skips on every current Driver.
fn autopkgtest_isolation_args(isolation: IsolationCapability) -> Vec<&'static str> {
    match isolation {
        IsolationCapability::None => vec![],
        IsolationCapability::Container => vec!["--ignore-restrictions", "isolation-container"],
        IsolationCapability::Machine => {
            vec![
                "--ignore-restrictions",
                "isolation-container,isolation-machine",
            ]
        }
    }
}

pub fn run_test(intent: &TestIntent) -> anyhow::Result<TestOutcome> {
    let package = load_package(&intent.source_dir)?;
    let registry = Registry::open_default_or_ephemeral()?;

    // `--changes` overrides only the artifact path. Driver and distro still
    // default to the prior binary-build Invocation when one exists.
    let invocations = registry
        .binary_build_changes_for_tree(&intent.source_dir)
        .unwrap_or_else(|error| {
            eprintln!("Warning: cannot read prior binary builds from the registry: {error}");
            Vec::new()
        });
    let prior_invocation = prior_binary_invocation(
        &invocations,
        intent.distro.as_deref(),
        intent.changes.is_some(),
    )?;

    let changes_path = if let Some(ref explicit) = intent.changes {
        if !explicit.is_file() {
            bail!("--changes file {} does not exist", explicit.display());
        }
        explicit.clone()
    } else {
        prior_invocation
            .as_ref()
            .and_then(|invocation| invocation.changes_path.clone())
            .context(
                "no prior binary build found; run `debmagic build binary` first or pass --changes",
            )?
    };
    if let Some(invocation) = prior_invocation.as_ref() {
        warn_on_prior_build_mismatch(intent, &package, invocation, &changes_path);
    }
    let exported_test_dir = exported_test_dir(&changes_path)?;
    ensure_exported_test_dir_replaceable(&exported_test_dir)?;

    let driver = intent
        .driver
        .or_else(|| {
            prior_invocation
                .as_ref()
                .and_then(|invocation| invocation.driver)
        })
        .ok_or_else(|| {
            anyhow!(
                "no driver specified and no prior build found; pass --driver or run `debmagic build binary` first"
            )
        })?;

    if driver == DriverType::Bare && !intent.allow_host_test {
        bail!(
            "the bare driver runs autopkgtest as root directly on the host; \
             pass --allow-host-test to opt in explicitly"
        );
    }

    let distro = if let Some(ref override_distro) = intent.distro {
        lookup_distro_override(override_distro, driver, &intent.config.driver)?
    } else if let Some(ref invocation) = prior_invocation {
        // The Invocation recorded the full DistroVersion, so custom suites
        // from a Driver's base_images work without re-resolving.
        invocation.distro.clone()
    } else {
        bail!(
            "no prior build metadata found; pass --distro when using --changes without a prior Invocation"
        );
    };

    let environment = Environment::new(
        driver,
        package.name(),
        &format!("{}-{}", package.name(), package.version()),
        &intent.source_dir,
        distro,
        intent.config.driver.persistent,
        EnvironmentPurpose::Test,
        &intent.config.environments_dir,
    );
    let version = package.version().to_string();

    claim_command(
        &registry,
        environment,
        &intent.config.driver,
        &intent.driver_overrides,
        HostRootPolicy::Reset,
        |environment, _| {
            stage_source_tree(
                environment,
                BuildKind::Binary,
                &package,
                intent.config.source_sync_mode,
                false,
            )?;
            copy_changes_artifacts(&changes_path, &environment.work_dir())
        },
        InvocationKind::TestRun,
        &version,
        |claimed| {
            crate::output::stage("Installing autopkgtest");
            let staged_source_dir = stage_dir(claimed.environment(), BuildKind::Binary);
            claimed.driver().run_command_checked(
                &["apt-get", "install", "-y", "autopkgtest"],
                &staged_source_dir,
                true,
                &[("DEBIAN_FRONTEND", "noninteractive")],
            )?;

            let work_dir = claimed.environment().work_dir();
            let changes_filename = changes_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow!("invalid .changes path: {}", changes_path.display()))?;
            let source_tree_name = staged_source_dir
                .file_name()
                .and_then(|name| name.to_str())
                .context("stage dir has no file name")?;
            let autopkgtest_out_host = claimed.environment().root_dir.join("autopkgtest-out");
            if autopkgtest_out_host.exists() {
                fs::remove_dir_all(&autopkgtest_out_host)?;
            }
            let output_dir_arg = "../autopkgtest-out";
            let summary_arg = "../autopkgtest-out/summary";

            // Binary-only builds have no .dsc in the .changes; pass the staged source
            // tree alongside the .changes so debian/tests/ is found without rebuilding
            // (-B). See autopkgtest(1) "TESTING A DEBIAN PACKAGE" (.changes + tree).
            // IsolationCapabilities become `--ignore-restrictions` (autopkgtest args,
            // not virt-null `--fake-capability`; see autopkgtest_isolation_args).
            let output_dir_flag = format!("--output-dir={output_dir_arg}");
            let summary_flag = format!("--summary={summary_arg}");
            let source_tree_arg = format!("{source_tree_name}/");
            let isolation_args =
                autopkgtest_isolation_args(claimed.driver().isolation_capability());
            let mut autopkgtest_cmd = vec!["autopkgtest", "-B", "--no-auto-control"];
            autopkgtest_cmd.extend(isolation_args);
            autopkgtest_cmd.extend([
                output_dir_flag.as_str(),
                summary_flag.as_str(),
                changes_filename,
                source_tree_arg.as_str(),
                "--",
                "null",
            ]);

            crate::output::stage(&format!("Running autopkgtest for {}", package.name()));
            let exit_code = claimed
                .driver()
                .run_command(&autopkgtest_cmd, &work_dir, true, &[], Capture::NONE)
                .map(|result| result.exit_code)
                .unwrap_or(-1);

            let summary_path = autopkgtest_out_host.join("summary");
            print_autopkgtest_notices(exit_code, &summary_path);

            replace_exported_test_dir(&exported_test_dir)?;
            copy_dir_all(&autopkgtest_out_host, &exported_test_dir).with_context(|| {
                format!(
                    "failed to copy test output to {}",
                    exported_test_dir.display()
                )
            })?;
            fs::write(exported_test_dir.join(TEST_OUTPUT_MARKER), "")?;
            println!("Test output written to {}", exported_test_dir.display());

            let outcome = map_autopkgtest_exit(exit_code, intent.strict);
            let failure_lines = if outcome == TestOutcome::Failed {
                vec![
                    format!("Tests failed (autopkgtest exit code {exit_code})."),
                    format!("Test logs: {}", exported_test_dir.display()),
                ]
            } else {
                Vec::new()
            };
            Ok(CommandCompletion {
                value: Ok(outcome),
                success: outcome == TestOutcome::Passed,
                shell_worthy: outcome == TestOutcome::Failed,
                finish_error_fails_command: false,
                changes_path: None,
                failure_lines,
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::Persistence;

    #[test]
    fn exported_test_dir_is_named_after_the_changes_file() -> anyhow::Result<()> {
        assert_eq!(
            exported_test_dir(Path::new("/out/pkg_1.0-1_amd64.changes"))?,
            PathBuf::from("/out/pkg_1.0-1_amd64.test")
        );
        Ok(())
    }

    #[test]
    fn foreign_test_dir_is_never_removed() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("debmagic-test-{}", uuid::Uuid::new_v4()));
        let foreign = dir.join("pkg.test");
        fs::create_dir_all(&foreign)?;
        fs::write(foreign.join("keep"), "user data")?;
        assert!(replace_exported_test_dir(&foreign).is_err());
        assert!(foreign.join("keep").is_file());

        fs::write(foreign.join(TEST_OUTPUT_MARKER), "")?;
        replace_exported_test_dir(&foreign)?;
        assert!(!foreign.exists());
        fs::remove_dir_all(&dir)?;
        Ok(())
    }

    #[test]
    fn test_environment_id_differs_from_build() {
        let distro = DistroVersion::new(debmagic_common::distro::Distro::Debian, "trixie", "13");
        let envs = PathBuf::from("/envs");
        let source_dir = PathBuf::from("/src");
        let build = Environment::new(
            DriverType::Docker,
            "pkg",
            "pkg-1.0-1",
            &source_dir,
            distro.clone(),
            Persistence::Always,
            EnvironmentPurpose::Build,
            &envs,
        );
        let test = Environment::new(
            DriverType::Docker,
            "pkg",
            "pkg-1.0-1",
            &source_dir,
            distro,
            Persistence::Always,
            EnvironmentPurpose::Test,
            &envs,
        );
        assert_ne!(build.id(), test.id());
        assert_ne!(build.root_dir, test.root_dir);
    }

    #[test]
    fn map_exit_pass() {
        assert_eq!(
            map_autopkgtest_exit(AUTOPKGTEST_EXIT_PASS, false),
            TestOutcome::Passed
        );
        assert_eq!(
            map_autopkgtest_exit(AUTOPKGTEST_EXIT_PASS, true),
            TestOutcome::Passed
        );
    }

    #[test]
    fn map_exit_fail_and_testbed_failure() {
        for code in [
            AUTOPKGTEST_EXIT_FAIL,
            6,
            AUTOPKGTEST_EXIT_ERRONEOUS_PKG,
            14,
            AUTOPKGTEST_EXIT_TESTBED_FAILURE,
            AUTOPKGTEST_EXIT_OTHER,
        ] {
            assert_eq!(
                map_autopkgtest_exit(code, false),
                TestOutcome::Failed,
                "code {code}"
            );
            assert_eq!(
                map_autopkgtest_exit(code, true),
                TestOutcome::Failed,
                "code {code} strict"
            );
        }
    }

    #[test]
    fn map_exit_spawn_failure_is_failed() {
        assert_eq!(map_autopkgtest_exit(-1, false), TestOutcome::Failed);
    }

    #[test]
    fn map_exit_skip_and_no_tests_respects_strict() {
        assert_eq!(
            map_autopkgtest_exit(AUTOPKGTEST_EXIT_SKIP, false),
            TestOutcome::Passed
        );
        assert_eq!(
            map_autopkgtest_exit(AUTOPKGTEST_EXIT_SKIP, true),
            TestOutcome::StrictFailure
        );
        assert_eq!(
            map_autopkgtest_exit(AUTOPKGTEST_EXIT_NO_TESTS, false),
            TestOutcome::Passed
        );
        assert_eq!(
            map_autopkgtest_exit(AUTOPKGTEST_EXIT_NO_TESTS, true),
            TestOutcome::StrictFailure
        );
    }

    #[test]
    fn autopkgtest_isolation_args_ignore_provided_rungs_only() {
        assert_eq!(
            autopkgtest_isolation_args(IsolationCapability::None),
            [] as [&str; 0]
        );
        assert_eq!(
            autopkgtest_isolation_args(IsolationCapability::Container),
            ["--ignore-restrictions", "isolation-container"]
        );
        assert_eq!(
            autopkgtest_isolation_args(IsolationCapability::Machine),
            [
                "--ignore-restrictions",
                "isolation-container,isolation-machine"
            ]
        );
    }

    #[test]
    fn autopkgtest_isolation_args_never_use_fake_capability() {
        for isolation in [
            IsolationCapability::None,
            IsolationCapability::Container,
            IsolationCapability::Machine,
        ] {
            let args = autopkgtest_isolation_args(isolation);
            assert!(
                !args.iter().any(|arg| arg.contains("fake-capability")),
                "virt-null --fake-capability isolation-* crashes autopkgtest 5.49+ (downtmp_prefix assert); isolation={isolation:?} args={args:?}"
            );
        }
    }
}
