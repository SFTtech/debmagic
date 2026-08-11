use std::path::PathBuf;

use anyhow::Context;

use crate::{
    build::{common::BuildDriverType, config::DriverOverrides},
    build_intent::load_config,
    config::Config,
};

/// Clap-free inputs for resolving a [`TestIntent`].
#[derive(Debug, Clone)]
pub struct TestIntentInput {
    /// Directory used when `source_dir` is unset (typically cwd).
    pub fallback_dir: PathBuf,
    pub source_dir: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub driver: Option<BuildDriverType>,
    pub persistent: Option<bool>,
    pub strict: bool,
    pub changes: Option<PathBuf>,
    pub allow_host_test: bool,
    pub shell_on_failure: bool,
    pub distro: Option<String>,
    pub driver_overrides: DriverOverrides,
}

/// Fully resolved description of *how* a TestRun executes.
///
/// Does not include *which* artifacts are being tested.
#[derive(Debug, Clone)]
pub struct TestIntent {
    pub source_dir: PathBuf,
    pub driver: Option<BuildDriverType>,
    pub strict: bool,
    pub changes: Option<PathBuf>,
    pub allow_host_test: bool,
    pub shell_on_failure: bool,
    pub distro: Option<String>,
    pub config: Config,
    pub driver_overrides: DriverOverrides,
}

pub fn resolve_test_intent(input: TestIntentInput) -> anyhow::Result<TestIntent> {
    let source_dir = std::path::absolute(input.source_dir.unwrap_or(input.fallback_dir))
        .context("resolving source dir failed")?;

    let mut config = load_config(Some(&source_dir), input.config_file.as_deref())?;

    if let Some(persistent) = input.persistent {
        config.driver.persistent = persistent;
    }

    let changes = if let Some(changes) = input.changes {
        Some(std::path::absolute(changes).context("resolving --changes path failed")?)
    } else {
        None
    };

    Ok(TestIntent {
        source_dir,
        driver: input.driver,
        strict: input.strict,
        changes,
        allow_host_test: input.allow_host_test,
        shell_on_failure: input.shell_on_failure,
        distro: input.distro,
        config,
        driver_overrides: input.driver_overrides,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{
        driver_bare::DriverBareConfigOverrides, driver_docker::DriverDockerConfigOverrides,
        driver_lxd::DriverLxdConfigOverrides,
    };

    fn asset_config() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets")
            .join("config1.toml")
    }

    fn base_input(fallback: PathBuf) -> TestIntentInput {
        TestIntentInput {
            fallback_dir: fallback,
            source_dir: None,
            config_file: Some(asset_config()),
            driver: None,
            persistent: None,
            strict: false,
            changes: None,
            allow_host_test: false,
            shell_on_failure: false,
            distro: None,
            driver_overrides: DriverOverrides {
                apt_mirror: None,
                proposed: None,
                docker: DriverDockerConfigOverrides { base_image: None },
                bare: DriverBareConfigOverrides {},
                lxd: DriverLxdConfigOverrides {
                    base_image: None,
                    project: None,
                },
            },
        }
    }

    #[test]
    fn resolve_applies_persistent_override() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        input.persistent = Some(false);

        let intent = resolve_test_intent(input)?;
        assert!(!intent.config.driver.persistent);
        Ok(())
    }

    #[test]
    fn resolve_passes_through_strict() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        input.strict = true;

        let intent = resolve_test_intent(input)?;
        assert!(intent.strict);
        Ok(())
    }

    #[test]
    fn resolve_passes_through_shell_on_failure() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        input.shell_on_failure = true;

        let intent = resolve_test_intent(input)?;
        assert!(intent.shell_on_failure);
        Ok(())
    }

    #[test]
    fn resolve_absolutizes_source_dir() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let intent = resolve_test_intent(base_input(dir.clone()))?;
        assert!(intent.source_dir.is_absolute());
        assert_eq!(intent.source_dir, std::path::absolute(&dir)?);
        Ok(())
    }
}
