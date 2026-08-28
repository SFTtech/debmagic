use std::path::PathBuf;

use crate::build::source::SourceSyncMode;
use crate::driver::config::DriverConfig;
use crate::signing::SignWith;
use anyhow::{Context, anyhow};
use config::{Config as ConfigBuilder, File};
use serde::Deserialize;

/// documented in docs/usage/config.md
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    pub driver: DriverConfig,
    pub temp_build_dir: PathBuf,
    pub incremental: bool,
    /// Which source files are staged into the build tree.
    pub source_sync_mode: SourceSyncMode,
    /// Always build the automatic `-dbgsym` debug symbol package.
    pub build_debug_symbols: bool,
    /// Signing of the resulting `.changes`/`.dsc`.
    pub sign: SignConfig,
    /// Run `debian/rules clean` before building (like `dpkg-buildpackage`
    /// does unless passed `-nc`). Disabled by default because non-incremental
    /// builds already stage a clean source tree and incremental builds preserve
    /// outputs intentionally.
    pub clean: bool,
    /// On build or test failure, drop into an interactive shell in the
    /// environment when stdout is a TTY.
    pub shell_on_failure: bool,
    /// Build for a dpkg architecture variant (e.g. `amd64v3` on Ubuntu),
    /// exported as `DEB_HOST_ARCH_VARIANT` for the build.
    pub host_arch_variant: Option<String>,
}

/// `[sign]` section: whether and how to sign the build artifacts.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct SignConfig {
    /// Sign the source package (`.changes`/`.dsc`) with `debsign` after
    /// building.
    pub source: bool,
    /// Where `debsign` runs: on the host or inside a minimal same-distro
    /// container with the host's gpg-agent socket forwarded in.
    pub with: SignWith,
    /// GPG key ID/email to sign with (debsign's `-k` option). `None` lets
    /// debsign fall back to its own maintainer-based key lookup, but
    /// container signing requires an explicit key.
    pub key: Option<String>,
    /// Send a desktop notification via `notify-send` just before `debsign`
    /// runs, so a hardware-key touch prompt isn't missed.
    pub notify: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            driver: DriverConfig::default(),
            temp_build_dir: PathBuf::from("/tmp/debmagic"),
            incremental: false,
            source_sync_mode: SourceSyncMode::default(),
            build_debug_symbols: false,
            sign: SignConfig::default(),
            clean: false,
            shell_on_failure: false,
            host_arch_variant: None,
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
    use crate::driver::DriverType;

    #[test]
    fn it_loads_a_simple_config() -> Result<(), anyhow::Error> {
        let test_asset_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets");
        let cfg = Config::new(&vec![test_asset_dir.join("config1.toml")])?;
        assert_eq!(cfg.driver.default, Some(DriverType::Docker));
        assert!(cfg.driver.persistent);

        assert!(
            cfg.driver.docker.base_images.get("debian:trixie")
                == Some(&"some-debian-trixie-image:latest".to_string())
        );

        Ok(())
    }

    #[test]
    fn it_loads_the_sign_section() -> Result<(), anyhow::Error> {
        let dir = std::env::temp_dir().join(format!("debmagic-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let file = dir.join("sign.toml");
        std::fs::write(
            &file,
            "[sign]\nsource = true\nwith = \"separate\"\nkey = \"you@example.com\"\nnotify = true\n",
        )?;
        let cfg = Config::new(&vec![file.clone()])?;
        std::fs::remove_dir_all(&dir).ok();
        assert!(cfg.sign.source);
        assert_eq!(cfg.sign.with, SignWith::Separate);
        assert_eq!(cfg.sign.key.as_deref(), Some("you@example.com"));
        assert!(cfg.sign.notify);
        Ok(())
    }
}
