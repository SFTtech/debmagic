use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::{
    build::{common::BuildDriverType, config::DriverConfig},
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
    pub persist_driver: Option<bool>,
    pub incremental: Option<bool>,
    pub docker_base_image: Option<String>,
}

/// Fully resolved description of *how* a package build should run.
#[derive(Debug, Clone)]
pub struct BuildIntent {
    pub source_dir: PathBuf,
    pub output_dir: PathBuf,
    pub driver: BuildDriverType,
    pub driver_config: DriverConfig,
    pub incremental: bool,
    pub temp_build_dir: PathBuf,
    /// One-shot docker base image override; applied when the target distro is known.
    pub docker_base_image: Option<String>,
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

    if let Some(persist_driver) = input.persist_driver {
        config.driver.persistent = persist_driver;
    }

    if let Some(incremental) = input.incremental {
        config.incremental = incremental;
    }
    if config.incremental {
        config.driver.persistent = true;
    }

    Ok(BuildIntent {
        source_dir,
        output_dir,
        driver: input.driver,
        driver_config: config.driver,
        incremental: config.incremental,
        temp_build_dir: config.temp_build_dir,
        docker_base_image: input.docker_base_image,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
            persist_driver: None,
            incremental: None,
            docker_base_image: None,
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
        input.persist_driver = Some(false);
        input.incremental = Some(true);

        let intent = resolve_build_intent(input)?;
        assert!(intent.incremental);
        assert!(intent.driver_config.persistent);
        Ok(())
    }

    #[test]
    fn resolve_honours_persist_driver_without_incremental() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        // config1.toml has persistent = true; CLI can turn it off
        input.persist_driver = Some(false);
        input.incremental = Some(false);

        let intent = resolve_build_intent(input)?;
        assert!(!intent.incremental);
        assert!(!intent.driver_config.persistent);
        Ok(())
    }

    #[test]
    fn resolve_keeps_docker_base_image_override() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        input.docker_base_image = Some("custom:image".to_string());

        let intent = resolve_build_intent(input)?;
        assert_eq!(intent.docker_base_image.as_deref(), Some("custom:image"));
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
}
