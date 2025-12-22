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
    pub fn new(cli_args: &Cli) -> anyhow::Result<Self> {
        let mut builder = ConfigBuilder::builder();

        let xdg_config_file = dirs::config_dir()
            .map(|p| p.join("debmagic").join("config.toml"))
            .ok_or(anyhow!("Could not determine user config directory"))?;
        if xdg_config_file.is_file() {
            let xdg_config_file = xdg_config_file.to_str();
            if let Some(xdg_config_file) = xdg_config_file {
                builder = builder.add_source(File::with_name(xdg_config_file));
            }
        }

        if let Some(config_override) = cli_args.config.as_ref().and_then(|f| f.to_str()) {
            builder = builder.add_source(File::with_name(config_override));
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
