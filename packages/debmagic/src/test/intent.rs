use std::path::PathBuf;

use anyhow::Context;

use crate::{
    config::Config,
    driver::{DriverType, Persistence, config::DriverOverrides},
};

/// Inputs for resolving a [`TestIntent`].
#[derive(Debug, Clone)]
pub struct TestIntentInput {
    /// Directory used when `source_dir` is unset (typically cwd).
    pub fallback_dir: PathBuf,
    pub source_dir: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub driver: Option<DriverType>,
    pub persistent: Option<Persistence>,
    pub strict: bool,
    pub changes: Option<PathBuf>,
    pub allow_host_test: bool,
    pub distro: Option<String>,
    pub driver_overrides: DriverOverrides,
}

/// Fully resolved description of *how* a TestRun executes.
///
/// Does not include *which* artifacts are being tested.
#[derive(Debug, Clone)]
pub struct TestIntent {
    pub source_dir: PathBuf,
    pub driver: Option<DriverType>,
    pub strict: bool,
    pub changes: Option<PathBuf>,
    pub allow_host_test: bool,
    pub distro: Option<String>,
    pub config: Config,
    pub driver_overrides: DriverOverrides,
}

pub fn resolve_test_intent(input: TestIntentInput) -> anyhow::Result<TestIntent> {
    // Canonicalized: Environment ids and registry lookups key on the
    // source_dir string, so all entry points must agree on one spelling.
    let source_dir = std::fs::canonicalize(input.source_dir.unwrap_or(input.fallback_dir))
        .context("resolving source dir failed")?;

    let mut config = Config::load(Some(&source_dir), input.config_file.as_deref())?;

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
        distro: input.distro,
        config,
        driver_overrides: input.driver_overrides,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{
        config::DriverOverrides, driver_bare::DriverBareConfigOverrides,
        driver_docker::DriverDockerConfigOverrides, driver_lxd::DriverLxdConfigOverrides,
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
            distro: None,
            driver_overrides: DriverOverrides {
                apt_mirror: None,
                proposed: None,
                apt_update_age: None,
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
        input.persistent = Some(Persistence::No);

        let intent = resolve_test_intent(input)?;
        assert_eq!(intent.config.driver.persistent, Persistence::No);
        Ok(())
    }

    #[test]
    fn resolve_absolutizes_source_dir() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let intent = resolve_test_intent(base_input(dir.clone()))?;
        assert!(intent.source_dir.is_absolute());
        assert_eq!(intent.source_dir, std::fs::canonicalize(&dir)?);
        Ok(())
    }
}
