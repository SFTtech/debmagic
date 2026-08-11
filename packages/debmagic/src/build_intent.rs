use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::{
    build::{
        common::{BuildDriverType, SourceSyncMode},
        config::DriverOverrides,
        signing::SignWith,
    },
    config::Config,
};

/// Clap-free inputs for resolving a [`BuildIntent`].
#[derive(Debug, Clone)]
pub struct BuildIntentInput {
    /// Directory used when `source_dir` / `output_dir` are unset (typically cwd).
    pub fallback_dir: PathBuf,
    pub source_dir: Option<PathBuf>,
    pub output_dir: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub driver: BuildDriverType,
    pub persistent: Option<bool>,
    pub incremental: Option<bool>,
    /// Force incremental off (e.g. source-only builds).
    pub disable_incremental: bool,
    pub debug_symbols: Option<bool>,
    pub sign: Option<bool>,
    pub no_sign: Option<bool>,
    pub sign_with: Option<SignWith>,
    pub sign_key: Option<String>,
    pub clean: Option<bool>,
    pub no_clean: Option<bool>,
    pub source_sync: Option<SourceSyncMode>,
    pub shell_on_failure: bool,
    pub driver_overrides: DriverOverrides,
}

/// Fully resolved description of *how* a package build should run.
///
/// Does not include *what* is being built (package identity or target distro).
#[derive(Debug, Clone)]
pub struct BuildIntent {
    pub source_dir: PathBuf,
    pub output_dir: PathBuf,
    pub driver: BuildDriverType,
    pub shell_on_failure: bool,
    pub config: Config,
    pub driver_overrides: DriverOverrides,
}

/// Precedence of config files is:
///
/// 1. explicit config file passed on the command line
/// 2. `<source_dir>/debian/debmagic.toml`
/// 3. `<XDG_CONFIG_HOME>/debmagic/config.toml`
pub fn load_config(
    source_dir: Option<&Path>,
    config_file: Option<&Path>,
) -> anyhow::Result<Config> {
    let mut config_file_paths = vec![];
    let xdg_config_file = dirs::config_dir().map(|p| p.join("debmagic").join("config.toml"));
    if let Some(xdg_config_file) = xdg_config_file
        && xdg_config_file.is_file()
    {
        config_file_paths.push(xdg_config_file);
    }

    if let Some(source_dir) = source_dir {
        config_file_paths.push(source_dir.join("debian").join("debmagic.toml"));
    }

    if let Some(config_file) = config_file {
        config_file_paths.push(config_file.to_path_buf());
    }

    Config::new(&config_file_paths)
}

pub fn resolve_build_intent(input: BuildIntentInput) -> anyhow::Result<BuildIntent> {
    let source_dir = std::path::absolute(input.source_dir.unwrap_or(input.fallback_dir.clone()))
        .context("resolving source dir failed")?;
    let output_dir = std::path::absolute(input.output_dir.unwrap_or(input.fallback_dir))
        .context("resolving output dir failed")?;

    let mut config = load_config(Some(&source_dir), input.config_file.as_deref())?;

    if let Some(persistent) = input.persistent {
        config.driver.persistent = persistent;
    }

    if input.disable_incremental {
        config.incremental = false;
    } else if let Some(incremental) = input.incremental {
        config.incremental = incremental;
    }

    if let Some(debug_symbols) = input.debug_symbols {
        config.build_debug_symbols = debug_symbols;
    }
    if let Some(sign) = input.sign {
        config.sign_package = sign;
    }
    if let Some(no_sign) = input.no_sign {
        config.sign_package = !no_sign;
    }
    if let Some(sign_with) = input.sign_with {
        config.sign_with = sign_with;
    }
    if let Some(sign_key) = input.sign_key {
        config.sign_key = Some(sign_key);
    }
    if let Some(clean) = input.clean {
        config.clean = clean;
    }
    if let Some(no_clean) = input.no_clean {
        config.clean = !no_clean;
    }
    if let Some(source_sync) = input.source_sync {
        config.source_sync_mode = source_sync;
    }
    if config.incremental {
        if config.clean {
            anyhow::bail!("incremental builds are incompatible with clean builds");
        }
        config.driver.persistent = true;
    }

    Ok(BuildIntent {
        source_dir,
        output_dir,
        driver: input.driver,
        shell_on_failure: input.shell_on_failure,
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

    fn base_input(fallback: PathBuf) -> BuildIntentInput {
        BuildIntentInput {
            fallback_dir: fallback,
            source_dir: None,
            output_dir: None,
            config_file: Some(asset_config()),
            driver: BuildDriverType::Docker,
            persistent: None,
            incremental: None,
            disable_incremental: false,
            debug_symbols: None,
            sign: None,
            no_sign: None,
            sign_with: None,
            sign_key: None,
            clean: None,
            no_clean: None,
            source_sync: None,
            shell_on_failure: false,
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
    fn load_config_reads_explicit_file() -> anyhow::Result<()> {
        let cfg = load_config(None, Some(&asset_config()))?;
        assert!(cfg.driver.persistent);
        assert_eq!(
            cfg.driver.docker.base_images.get("debian:trixie"),
            Some(&"some-debian-trixie-image:latest".to_string())
        );
        Ok(())
    }

    #[test]
    fn resolve_applies_incremental_implies_persistent() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        input.persistent = Some(false);
        input.incremental = Some(true);

        let intent = resolve_build_intent(input)?;
        assert!(intent.config.incremental);
        assert!(intent.config.driver.persistent);
        Ok(())
    }

    #[test]
    fn resolve_honours_persistent_without_incremental() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        // config1.toml has persistent = true; CLI can turn it off
        input.persistent = Some(false);
        input.incremental = Some(false);

        let intent = resolve_build_intent(input)?;
        assert!(!intent.config.incremental);
        assert!(!intent.config.driver.persistent);
        Ok(())
    }

    #[test]
    fn resolve_passes_through_shell_on_failure() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        input.shell_on_failure = true;

        let intent = resolve_build_intent(input)?;
        assert!(intent.shell_on_failure);
        Ok(())
    }

    #[test]
    fn resolve_keeps_docker_base_image_override() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        input.driver_overrides.docker.base_image = Some("custom:image".to_string());

        let intent = resolve_build_intent(input)?;
        assert_eq!(
            intent.driver_overrides.docker.base_image.as_deref(),
            Some("custom:image")
        );
        Ok(())
    }

    #[test]
    fn resolve_absolutizes_paths() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let intent = resolve_build_intent(base_input(dir.clone()))?;
        assert!(intent.source_dir.is_absolute());
        assert!(intent.output_dir.is_absolute());
        assert_eq!(intent.source_dir, std::path::absolute(&dir)?);
        assert_eq!(intent.output_dir, std::path::absolute(&dir)?);
        Ok(())
    }

    #[test]
    fn resolve_rejects_incremental_with_clean() {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        input.incremental = Some(true);
        input.clean = Some(true);

        let result = resolve_build_intent(input);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("incremental builds are incompatible with clean builds")
        );
    }

    #[test]
    fn resolve_disable_incremental_for_source_builds() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        input.incremental = Some(true);
        input.disable_incremental = true;

        let intent = resolve_build_intent(input)?;
        assert!(!intent.config.incremental);
        Ok(())
    }
}
