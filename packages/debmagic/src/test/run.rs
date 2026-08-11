use std::{
    fs, io,
    io::{BufReader, IsTerminal, stdout},
    path::{Path, PathBuf},
};

use super::intent::TestIntent;
use crate::build::config::DriverOverrides;
use crate::build::source::stage_source_tree;
use crate::package::PackageIdentity;
use crate::{
    build::{
        artifacts::{copy_changes_artifacts, copy_dir_all, find_changes_file},
        common::{BuildConfig, BuildDriver, BuildDriverType, BuildMetadata, EnvironmentPurpose},
        config::DriverConfig,
        remove_environment_root,
        signing::SignWith,
    },
    package::load_package_identity,
};
use anyhow::{Context, anyhow, bail};
use debmagic_common::distro::DistroVersion;

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

struct TestRun {
    config: BuildConfig,
    driver: Box<dyn BuildDriver>,
}

fn get_build_root_and_identifier(
    temp_build_dir: &Path,
    identity: &PackageIdentity,
) -> (String, PathBuf) {
    let package_identifier = format!("{}-{}", identity.name, identity.version);
    let build_root = temp_build_dir.join(&package_identifier);
    (package_identifier, build_root)
}

fn test_build_root(build_root: &Path) -> PathBuf {
    let package_identifier = build_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    build_root.with_file_name(format!("{package_identifier}-test"))
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

fn lookup_distro(name: &str) -> anyhow::Result<DistroVersion> {
    debmagic_common::distro::get_distro_version(name)
        .ok_or_else(|| anyhow!("unknown distro codename '{name}'"))
}

fn load_build_metadata(build_root: &Path) -> anyhow::Result<BuildMetadata> {
    let build_metadata_path = build_root.join("build.json");
    if !build_metadata_path.is_file() {
        bail!("No build.json found");
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .open(&build_metadata_path)?;
    let reader = BufReader::new(&file);
    serde_json::from_reader(reader).with_context(|| {
        format!(
            "Failed to read build metadata from {} - invalid json",
            build_metadata_path.display()
        )
    })
}

impl TestRun {
    fn create(
        config: &BuildConfig,
        driver_config: &DriverConfig,
        driver_overrides: &DriverOverrides,
    ) -> anyhow::Result<Self> {
        let driver = crate::build::get_build_driver(config, driver_config, driver_overrides)
            .context(format!("failed to create {:?} build driver", config.driver))?;
        Ok(Self {
            config: config.clone(),
            driver,
        })
    }

    fn write_metadata(&self) -> anyhow::Result<()> {
        let metadata = BuildMetadata {
            config: self.config.clone(),
            driver_metadata: self.driver.get_build_metadata(),
        };
        let path = self.config.build_root_dir.join("build.json");
        let json = serde_json::to_string_pretty(&metadata)
            .context("Failed to serialize build metadata")?;
        fs::write(path, json)?;
        Ok(())
    }
}

fn prepare_test_env(
    intent: &TestIntent,
    test_config: &BuildConfig,
    identity: &PackageIdentity,
    changes_path: &Path,
) -> anyhow::Result<TestRun> {
    let test_root = &test_config.build_root_dir;

    if intent.config.driver.persistent && test_root.exists() {
        let test_run =
            TestRun::create(test_config, &intent.config.driver, &intent.driver_overrides).context(
                format!("failed to create {:?} build driver", test_config.driver),
            )?;
        test_run
            .driver
            .reset_build_root()
            .context("failed to reset persistent test directory")?;
        test_config
            .create_dirs()
            .context("failed to create test directories")?;
        stage_source_tree(test_config, identity)?;
        copy_changes_artifacts(changes_path, &test_config.build_work_dir())?;
        return Ok(test_run);
    }

    remove_environment_root(test_root, &intent.config.driver)?;

    test_config
        .create_dirs()
        .context("failed to create test directories")?;
    stage_source_tree(test_config, identity)?;
    copy_changes_artifacts(changes_path, &test_config.build_work_dir())?;

    let test_run = TestRun::create(test_config, &intent.config.driver, &intent.driver_overrides)?;
    Ok(test_run)
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

pub fn run_test(intent: &TestIntent) -> anyhow::Result<TestOutcome> {
    let identity = load_package_identity(&intent.source_dir)?;
    let (package_identifier, build_root) =
        get_build_root_and_identifier(&intent.config.temp_build_dir, &identity);

    let changes_path = if let Some(ref explicit) = intent.changes {
        if !explicit.is_file() {
            bail!("--changes file {} does not exist", explicit.display());
        }
        explicit.clone()
    } else {
        let build_metadata_path = build_root.join("build.json");
        if !build_metadata_path.is_file() {
            bail!(
                "no prior build found at {}; run `debmagic build binary` first",
                build_root.display()
            );
        }
        find_changes_file(&build_root.join("work"))?
    };

    let prior_build = if build_root.join("build.json").is_file() {
        Some(load_build_metadata(&build_root)?)
    } else {
        None
    };

    let driver = intent
        .driver
        .or_else(|| prior_build.as_ref().map(|metadata| metadata.config.driver))
        .ok_or_else(|| {
            anyhow!(
                "no driver specified and no prior build found; pass --driver or run `debmagic build binary` first"
            )
        })?;

    if driver == BuildDriverType::Bare && !intent.allow_host_test {
        bail!(
            "the bare driver runs autopkgtest as root directly on the host; \
             pass --allow-host-test to opt in explicitly"
        );
    }

    let distro = if let Some(ref override_distro) = intent.distro {
        lookup_distro(override_distro)?
    } else if let Some(ref metadata) = prior_build {
        metadata.config.distro.clone()
    } else {
        bail!(
            "no prior build metadata found; pass --distro when using --changes without a build root"
        );
    };

    let test_root = test_build_root(&build_root);
    let output_dir = prior_build
        .as_ref()
        .map(|metadata| metadata.config.output_dir.clone())
        .unwrap_or_else(|| intent.source_dir.clone());

    let test_config = BuildConfig {
        driver,
        package_name: identity.name.clone(),
        package_identifier,
        source_dir: intent.source_dir.clone(),
        output_dir,
        build_root_dir: test_root.clone(),
        distro,
        sign_package: false,
        sign_with: SignWith::Auto,
        sign_key: None,
        build_debug_symbols: false,
        clean: false,
        persistent: intent.config.driver.persistent,
        incremental: false,
        source_sync_mode: intent.config.source_sync_mode,
        purpose: EnvironmentPurpose::Test,
    };

    let test_run = prepare_test_env(intent, &test_config, &identity, &changes_path)
        .context("failed to prepare test environment")?;
    test_run
        .write_metadata()
        .context("failed to write test metadata")?;

    let apt_env = [("DEBIAN_FRONTEND", "noninteractive")];
    test_run.driver.run_command_checked(
        &["apt-get", "update"],
        &test_config.build_source_dir(),
        true,
        &apt_env,
    )?;
    test_run.driver.run_command_checked(
        &["apt-get", "install", "-y", "autopkgtest"],
        &test_config.build_source_dir(),
        true,
        &apt_env,
    )?;

    let work_dir = test_config.build_work_dir();
    let changes_filename = changes_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("invalid .changes path: {}", changes_path.display()))?;
    let source_tree_name = test_config.package_identifier.as_str();
    let autopkgtest_out_host = test_root.join("autopkgtest-out");
    if autopkgtest_out_host.exists() {
        fs::remove_dir_all(&autopkgtest_out_host)?;
    }
    let output_dir_arg = "../autopkgtest-out";
    let summary_arg = "../autopkgtest-out/summary";

    // Binary-only builds have no .dsc in the .changes; pass the staged source
    // tree alongside the .changes so debian/tests/ is found without rebuilding
    // (-B). See autopkgtest(1) "TESTING A DEBIAN PACKAGE" (.changes + tree).
    let autopkgtest_cmd = [
        "autopkgtest",
        "-B",
        "--no-auto-control",
        &format!("--output-dir={output_dir_arg}"),
        &format!("--summary={summary_arg}"),
        changes_filename,
        &format!("{source_tree_name}/"),
        "--",
        "null",
    ];

    let exit_code = test_run
        .driver
        .run_command(&autopkgtest_cmd, &work_dir, true, &[])
        .unwrap_or(-1);

    let summary_path = autopkgtest_out_host.join("summary");
    print_autopkgtest_notices(exit_code, &summary_path);

    let export_root = prior_build
        .as_ref()
        .map(|_| build_root.as_path())
        .unwrap_or_else(|| changes_path.parent().unwrap());
    let exported_test_dir = export_root.join("test");
    if exported_test_dir.exists() {
        fs::remove_dir_all(&exported_test_dir)?;
    }
    copy_dir_all(&autopkgtest_out_host, &exported_test_dir).with_context(|| {
        format!(
            "failed to copy test output to {}",
            exported_test_dir.display()
        )
    })?;
    println!("Test output written to {}", exported_test_dir.display());

    let outcome = map_autopkgtest_exit(exit_code, intent.strict);

    if outcome == TestOutcome::Failed {
        eprintln!("Tests failed (autopkgtest exit code {exit_code}).");
        eprintln!("Test logs: {}", exported_test_dir.display());
        if intent.shell_on_failure && stdout().is_terminal() {
            eprintln!("Dropping into shell...");
            if let Err(shell_error) = test_run
                .driver
                .interactive_shell(&test_config.build_source_dir())
            {
                eprintln!("Dropping into shell failed: {shell_error}");
            }
        } else if intent.shell_on_failure {
            eprintln!(
                "--shell-on-failure is set but stdout is not a TTY; skipping interactive shell"
            );
        } else {
            eprintln!("Re-run with --shell-on-failure to inspect the test environment");
        }
    }

    if !test_config.persistent {
        // Clear container-owned files from the bind mount before destroying
        // the container; otherwise the host user cannot remove them later.
        if let Err(e) = test_run.driver.reset_build_root() {
            eprintln!("Warning: failed to reset test root before cleanup: {e}");
        }
    }

    if let Err(cleanup_error) = test_run.driver.cleanup() {
        eprintln!("Failed to clean up test environment: {cleanup_error}");
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::PackageIdentity;
    use debmagic_common::debian::version::PackageVersion;

    fn sample_identity() -> PackageIdentity {
        PackageIdentity {
            name: "pkg".to_string(),
            version: PackageVersion::new(None, "1.0".to_string(), Some("1".to_string())),
            source_dir: PathBuf::from("/src"),
        }
    }

    #[test]
    fn test_build_root_appends_test_suffix() {
        let (_, build_root) =
            get_build_root_and_identifier(Path::new("/tmp/debmagic"), &sample_identity());
        assert_eq!(build_root, PathBuf::from("/tmp/debmagic/pkg-1.0-1"));
        assert_eq!(
            test_build_root(&build_root),
            PathBuf::from("/tmp/debmagic/pkg-1.0-1-test")
        );
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
}
