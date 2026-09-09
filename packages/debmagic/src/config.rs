use std::path::{Path, PathBuf};

use crate::build::source::SourceSyncMode;
use crate::driver::config::DriverConfig;
use crate::sign::SignTool;
use anyhow::{Context, anyhow};
use config::{Config as ConfigBuilder, File};
use serde::{Deserialize, Serialize};

/// Outcome of looking at one candidate config file location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigPathStatus {
    /// The file exists and was loaded.
    Used,
    /// The location was considered but no file exists there.
    NotFound,
}

/// The user-global config file. `DEBMAGIC_CONFIG_GLOBAL` overrides the
/// default XDG location (like git's `GIT_CONFIG_GLOBAL`); `/dev/null`
/// disables it.
fn global_config_path() -> anyhow::Result<PathBuf> {
    if let Ok(path) = std::env::var("DEBMAGIC_CONFIG_GLOBAL") {
        return Ok(PathBuf::from(path));
    }

    dirs::config_dir()
        .map(|p| p.join("debmagic").join("config.toml"))
        .context("cannot determine the user config directory")
}

/// The in-package config file, always `<source_dir>/debian/debmagic.toml`.
fn project_config_path(source_dir: &Path) -> PathBuf {
    source_dir.join("debian").join("debmagic.toml")
}

/// The explicit `-c` config file; an error if it does not exist.
fn explicit_config_path(config_file: &Path) -> anyhow::Result<ConfigPath> {
    if !config_file.is_file() {
        anyhow::bail!("config file '{}' does not exist", config_file.display());
    }
    Ok(ConfigPath::new(
        ConfigLayer::Explicit,
        config_file.to_path_buf(),
        ConfigPathStatus::Used,
    ))
}

/// Which config layer a candidate file belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigLayer {
    Global,
    Project,
    Explicit,
}

/// A candidate config file location and whether it was loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPath {
    pub layer: ConfigLayer,
    pub path: PathBuf,
    pub status: ConfigPathStatus,
}

impl ConfigPath {
    fn new(layer: ConfigLayer, path: PathBuf, status: ConfigPathStatus) -> Self {
        Self {
            layer,
            path,
            status,
        }
    }

    pub fn is_used(&self) -> bool {
        self.status == ConfigPathStatus::Used
    }

    /// Write `key = value` into this file, preserving comments, formatting
    /// and all other entries. The value is parsed as TOML when valid,
    /// else treated as a string.
    pub fn set_value(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let parsed: toml_edit::Value = match value.parse::<toml_edit::Value>() {
            Ok(value) => value,
            Err(_) => toml_edit::Value::from(value),
        };

        let contents = std::fs::read_to_string(&self.path).unwrap_or_default();
        let mut doc = contents
            .parse::<toml_edit::DocumentMut>()
            .with_context(|| format!("parsing config file '{}' failed", self.path.display()))?;

        let parts: Vec<&str> = key.split('.').collect();
        let (leaf, parents) = parts.split_last().context("config key must not be empty")?;

        let mut current = doc.as_table_mut();
        for part in parents {
            current = current
                .entry(part)
                .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
                .as_table_mut()
                .with_context(|| format!("config key '{key}' collides with a non-table value"))?;
        }
        current.insert(leaf, toml_edit::value(parsed));

        std::fs::write(&self.path, doc.to_string())
            .with_context(|| format!("writing config file '{}' failed", self.path.display()))?;
        Ok(())
    }
}

/// Config file that `set` should target, selected from the candidate
/// locations: the explicit `-c` file, else with `global` the user-global
/// config file (created on demand), else the project `debian/debmagic.toml`
/// if it exists, else the user-global config file.
pub fn resolve_set_target(
    source_dir: Option<&Path>,
    config_file: Option<&Path>,
    global: bool,
) -> anyhow::Result<ConfigPath> {
    let paths = Config::resolve_paths(source_dir, config_file)?;

    if let Some(explicit) = paths.iter().find(|p| p.layer == ConfigLayer::Explicit) {
        return Ok(explicit.clone());
    }

    if !global
        && let Some(project) = paths
            .iter()
            .find(|p| p.layer == ConfigLayer::Project && p.is_used())
    {
        return Ok(project.clone());
    }

    let global_config = paths
        .iter()
        .find(|p| p.layer == ConfigLayer::Global)
        .context("the user-global config location is unknown")?;
    Ok(ConfigPath::new(
        ConfigLayer::Global,
        global_config.path.clone(),
        ConfigPathStatus::Used,
    ))
}

/// documented in docs/usage/config.md
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    pub driver: DriverConfig,
    pub temp_build_dir: PathBuf,
    /// Where build artifacts are exported; relative paths resolve against
    /// the package root.
    pub output_dir: PathBuf,
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
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct SignConfig {
    /// Sign the source package (`.changes`/`.dsc`) after building.
    pub source: bool,
    /// GPG key ID/email to sign with. `None` falls back to the
    /// `Changed-By:`/`Maintainer:` address of the file being signed.
    pub key: Option<String>,
    /// Which OpenPGP implementation to use.
    pub tool: SignTool,
    /// Custom signing command when `tool` is `custom`, like debsign's `-p`.
    pub command: Option<String>,
    /// Send a desktop notification via `notify-send` just before signing,
    /// so a hardware-key touch prompt isn't missed.
    pub notify: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            driver: DriverConfig::default(),
            temp_build_dir: PathBuf::from("/tmp/debmagic"),
            output_dir: PathBuf::from("build"),
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
    /// Precedence of config files is:
    ///
    /// 1. explicit config file passed on the command line
    /// 2. `<source_dir>/debian/debmagic.toml`
    /// 3. `<XDG_CONFIG_HOME>/debmagic/config.toml`
    ///
    /// An explicit config file that does not exist is an error; the other
    /// locations are optional and silently skipped when absent.
    pub fn resolve_paths(
        source_dir: Option<&Path>,
        config_file: Option<&Path>,
    ) -> anyhow::Result<Vec<ConfigPath>> {
        let mut paths = vec![];

        let global_config = global_config_path()?;
        let status = if global_config.is_file() {
            ConfigPathStatus::Used
        } else {
            ConfigPathStatus::NotFound
        };
        paths.push(ConfigPath::new(ConfigLayer::Global, global_config, status));

        if let Some(source_dir) = source_dir {
            let project_config = project_config_path(source_dir);
            let status = if project_config.is_file() {
                ConfigPathStatus::Used
            } else {
                ConfigPathStatus::NotFound
            };
            paths.push(ConfigPath::new(
                ConfigLayer::Project,
                project_config,
                status,
            ));
        }

        if let Some(config_file) = config_file {
            paths.push(explicit_config_path(config_file)?);
        }

        Ok(paths)
    }

    pub fn load(source_dir: Option<&Path>, config_file: Option<&Path>) -> anyhow::Result<Self> {
        let paths = Self::resolve_paths(source_dir, config_file)?;
        Self::new(&paths)
    }

    pub fn new(config_files: &[ConfigPath]) -> anyhow::Result<Self> {
        let mut builder = ConfigBuilder::builder();

        for file in config_files.iter().filter(|f| f.is_used()) {
            builder = builder.add_source(File::with_name(&file.path.to_string_lossy()));
        }

        let build = builder
            .build()
            .context("Failed to initialize config reader")?;
        let config: anyhow::Result<Self> = build
            .try_deserialize()
            .map_err(|e| anyhow!("Failed to read config: {e}"));

        config
    }

    /// Look up a dotted key (e.g. `sign.key`) in the serialized config,
    /// returning the value as a TOML fragment.
    pub fn get_value(&self, key: &str) -> anyhow::Result<String> {
        let table: toml::Table =
            toml::from_str(&toml::to_string(self)?).context("serializing config failed")?;

        let mut current = &toml::Value::Table(table);
        for part in key.split('.') {
            current = current
                .as_table()
                .and_then(|t| t.get(part))
                .with_context(|| format!("no such config key: '{key}'"))?;
        }

        match current {
            toml::Value::String(s) => Ok(s.clone()),
            toml::Value::Integer(i) => Ok(i.to_string()),
            toml::Value::Float(f) => Ok(f.to_string()),
            toml::Value::Boolean(b) => Ok(b.to_string()),
            toml::Value::Datetime(d) => Ok(d.to_string()),
            value => Ok(toml::to_string(value)?),
        }
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
        let cfg = Config::new(&[ConfigPath::new(
            ConfigLayer::Explicit,
            test_asset_dir.join("config1.toml"),
            ConfigPathStatus::Used,
        )])?;
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
            "[sign]\nsource = true\nkey = \"you@example.com\"\ncommand = \"gpg --foo\"\nnotify = true\n",
        )?;
        let cfg = Config::new(&[ConfigPath::new(
            ConfigLayer::Explicit,
            file.clone(),
            ConfigPathStatus::Used,
        )])?;
        std::fs::remove_dir_all(&dir).ok();
        assert!(cfg.sign.source);
        assert_eq!(cfg.sign.key.as_deref(), Some("you@example.com"));
        assert_eq!(cfg.sign.command.as_deref(), Some("gpg --foo"));
        assert!(cfg.sign.notify);
        Ok(())
    }
}
