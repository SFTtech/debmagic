use std::path::PathBuf;

use crate::build::common::SourceSyncMode;
use crate::build::config::DriverConfig;
use crate::build::signing::SignWith;
use anyhow::{Context, anyhow};
use config::{Config as ConfigBuilder, File};
use serde::Deserialize;

/// documented in docs/usage/config.md
#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub driver: DriverConfig,
    pub temp_build_dir: PathBuf,
    pub incremental: bool,
    /// Which source files are staged into the build tree.
    pub source_sync_mode: SourceSyncMode,
    /// Always build the automatic `-dbgsym` debug symbol package.
    pub build_debug_symbols: bool,
    /// Sign the resulting `.changes`/`.dsc` with `debsign` after building.
    pub sign_package: bool,
    /// Where `debsign` runs: on the host or inside a minimal same-distro
    /// container with the host's gpg-agent socket forwarded in.
    pub sign_with: SignWith,
    /// GPG key ID/email to sign with (debsign's `-k` option). `None` lets
    /// debsign fall back to its own maintainer-based key lookup, but
    /// container signing requires an explicit key.
    pub sign_key: Option<String>,
    /// Run `debian/rules clean` before building (like `dpkg-buildpackage`
    /// does unless passed `-nc`). Disabled by default because non-incremental
    /// builds already stage a clean source tree and incremental builds preserve
    /// outputs intentionally.
    pub clean: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            driver: DriverConfig::default(),
            temp_build_dir: PathBuf::from("/tmp/debmagic"),
            incremental: false,
            source_sync_mode: SourceSyncMode::default(),
            build_debug_symbols: false,
            sign_package: false,
            sign_with: SignWith::default(),
            sign_key: None,
            clean: false,
        }
    }
}

impl Config {
    pub fn new(config_files: &Vec<PathBuf>) -> anyhow::Result<Self> {
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
    use super::*;

    #[test]
    fn it_loads_a_simple_config() -> Result<(), anyhow::Error> {
        let test_asset_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets");
        let cfg = Config::new(&vec![test_asset_dir.join("config1.toml")])?;
        assert!(cfg.driver.persistent);

        assert!(
            cfg.driver.docker.base_images.get("debian:trixie")
                == Some(&"some-debian-trixie-image:latest".to_string())
        );

        Ok(())
    }
}
