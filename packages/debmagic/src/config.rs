use std::path::PathBuf;

use crate::{build::config::DriverConfig, cli::Cli};
use anyhow::{Context, anyhow};
use config::{Config as ConfigBuilder, File};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub driver: DriverConfig,
    pub temp_build_dir: PathBuf,
    pub dry_run: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            driver: DriverConfig::default(),
            temp_build_dir: PathBuf::from("/tmp/debmagic"),
            dry_run: false,
        }
    }
}

impl Config {
    pub fn new(config_files: &Vec<PathBuf>, _cli_args: &Cli) -> anyhow::Result<Self> {
        let mut builder = ConfigBuilder::builder();

        for file in config_files {
            if file.is_file() {
                builder = builder.add_source(File::with_name(&file.to_string_lossy()));
            }
        }

        // TODO: reimplement cli arg overwrites
        let build = builder
            .build()
            .context("Failed to initialize config reader")?;
        let config: anyhow::Result<Self> = build
            .try_deserialize()
            .map_err(|e| anyhow!("Failed to read config: {e}"));
        config
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::Commands;

    use super::*;

    #[test]
    fn it_loads_a_simple_config() -> Result<(), anyhow::Error> {
        let test_asset_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets");
        let cfg = Config::new(
            &vec![test_asset_dir.join("config1.toml")],
            &Cli {
                config: None,
                command: Commands::Version {},
            },
        )?;
        assert!(cfg.dry_run);

        Ok(())
    }
}
