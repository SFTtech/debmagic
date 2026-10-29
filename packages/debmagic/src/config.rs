use std::path::{Path, PathBuf};

use crate::apt::AptConfig;
use crate::build::source::SourceSyncMode;
use crate::driver::DriverType;
use crate::driver::config::DriverConfig;
use crate::output_dir::OutputDirClash;
use crate::sign::{SignMode, SignTool};
use crate::upload::UploadConfig;
use crate::upstream::orig::OrigTarballConfig;
use anyhow::{Context, anyhow, bail};
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
    // unit tests must not depend on the developer's config
    if cfg!(test) {
        return Ok(PathBuf::from("/dev/null"));
    }
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
/// config file, else the project `debian/debmagic.toml`. The global and
/// project files are created on demand.
pub fn resolve_set_target(
    source_dir: Option<&Path>,
    config_file: Option<&Path>,
    global: bool,
) -> anyhow::Result<ConfigPath> {
    let paths = Config::resolve_paths(source_dir, config_file)?;

    if let Some(explicit) = paths.iter().find(|p| p.layer == ConfigLayer::Explicit) {
        return Ok(explicit.clone());
    }

    let layer = if global {
        ConfigLayer::Global
    } else {
        ConfigLayer::Project
    };
    let target = paths
        .iter()
        .find(|p| p.layer == layer)
        .with_context(|| format!("the {layer:?} config location is unknown"))?;
    Ok(ConfigPath::new(
        layer,
        target.path.clone(),
        ConfigPathStatus::Used,
    ))
}

/// Inputs for the `debmagic config` command.
#[derive(Debug, Clone)]
pub struct ConfigCommand {
    pub source_dir: PathBuf,
    pub config_file: Option<PathBuf>,
    pub command: ConfigCommandKind,
}

/// Which `debmagic config` subcommand to run.
#[derive(Debug, Clone)]
pub enum ConfigCommandKind {
    Show,
    /// `config get <key>`
    Get {
        key: String,
    },
    /// `config set <key> <value>`
    Set {
        key: String,
        value: String,
        /// Write to the user-wide config file instead of the project's.
        global: bool,
    },
}

/// Run the `debmagic config` command: show, get or set config values.
pub fn run_config(command: ConfigCommand) -> anyhow::Result<()> {
    let source_dir = command.source_dir;
    match &command.command {
        ConfigCommandKind::Show => {
            let paths = Config::resolve_paths(Some(&source_dir), command.config_file.as_deref())?;

            eprintln!("debmagic: config files (highest precedence first):");
            for entry in &paths {
                let status = match entry.status {
                    ConfigPathStatus::Used => "used",
                    ConfigPathStatus::NotFound => "not found",
                };
                eprintln!("debmagic:   {status:<10} {}", entry.path.display());
            }

            let config = Config::new(&paths)?;
            let effective_driver = config.driver.default.unwrap_or(DriverType::Bare);
            eprintln!("debmagic: using driver: {effective_driver} (cfg: driver.default)");
            print!("{}", toml::to_string_pretty(&config)?);
        }
        ConfigCommandKind::Get { key } => {
            let config = Config::load(Some(&source_dir), command.config_file.as_deref())?;
            println!("{}", config.get_value(key)?);
        }
        ConfigCommandKind::Set { key, value, global } => {
            let target =
                resolve_set_target(Some(&source_dir), command.config_file.as_deref(), *global)?;

            if let Some(parent) = target.path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {} failed", parent.display()))?;
            }

            target.set_value(key, value)?;

            // validate the result parses and the key landed
            let config = Config::load(Some(&source_dir), command.config_file.as_deref())?;
            let effective = config.get_value(key)?;
            println!("debmagic: {} = {}", key, effective.trim_end());
            eprintln!("debmagic: written to {}", target.path.display());
        }
    }
    Ok(())
}

/// documented in docs/usage/config.md
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    pub driver: DriverConfig,
    /// `[apt]` section.
    pub apt: AptConfig,
    /// Parent directory of Environment Host roots.
    pub environments_dir: PathBuf,
    /// Where build artifacts are exported; relative paths resolve against
    /// the package source directory.
    pub output_dir: PathBuf,
    /// Used instead of `output_dir` when that holds upstream's files.
    /// Relative paths resolve against the package source directory.
    pub output_dir_fallback: PathBuf,
    /// How to handle an output dir clash with upstream's files.
    pub output_dir_clash: OutputDirClash,
    pub incremental: bool,
    /// Which source files are staged into the build tree.
    pub source_sync_mode: SourceSyncMode,
    /// Always build the automatic `-dbgsym` debug symbol package.
    pub build_debug_symbols: bool,
    /// Run the package's test suite during the build. When false, exports
    /// `DEB_BUILD_OPTIONS=nocheck` so dpkg-buildpackage skips tests.
    pub run_test: bool,
    /// Signing of the resulting `.changes`/`.dsc`.
    pub sign: SignConfig,
    /// Named upload targets for `debmagic upload`.
    pub upload: UploadConfig,
    /// How to fetch the `orig` tarball for source builds.
    pub orig_tarball: OrigTarballConfig,
    /// `upstream` command behavior.
    pub upstream: UpstreamConfig,
    /// Run `debian/rules clean` before building (like `dpkg-buildpackage`
    /// does unless passed `-nc`). Disabled by default because non-incremental
    /// builds already stage a clean source tree and incremental builds preserve
    /// outputs intentionally.
    pub clean: bool,
    /// Default for the `build`, `test` and `sign` notifications.
    pub notify: bool,
    /// `[build]` section.
    pub build: BuildConfig,
    /// `[test]` section.
    pub test: TestConfig,
    /// Build for a dpkg architecture variant (e.g. `amd64v3` on Ubuntu),
    /// exported as `DEB_HOST_ARCH_VARIANT` for the build.
    pub host_arch_variant: Option<String>,
}

/// `[build]` section: `debmagic build` behavior.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct BuildConfig {
    /// Desktop notification and terminal bell when the build finishes;
    /// the top-level `notify` when unset.
    pub notify: Option<bool>,
    /// Sign source builds without `--sign`, when signing is configured.
    pub sign_source: bool,
    /// Sign binary builds without `--sign`, when signing is configured.
    pub sign_binary: bool,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            notify: None,
            sign_source: true,
            sign_binary: false,
        }
    }
}

/// `[test]` section: `debmagic test` behavior.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct TestConfig {
    /// Desktop notification and terminal bell when the tests finish;
    /// the top-level `notify` when unset.
    pub notify: Option<bool>,
}

impl Config {
    pub fn build_notify(&self) -> bool {
        self.build.notify.unwrap_or(self.notify)
    }

    pub fn test_notify(&self) -> bool {
        self.test.notify.unwrap_or(self.notify)
    }

    pub fn sign_notify(&self) -> bool {
        self.sign.notify.unwrap_or(self.notify)
    }

    /// Apply `--notify`: it covers every step of the command.
    pub fn override_notify(&mut self, notify: Option<bool>) {
        if let Some(notify) = notify {
            self.notify = notify;
            self.build.notify = Some(notify);
            self.test.notify = Some(notify);
            self.sign.notify = Some(notify);
        }
    }
}

/// `[upstream]` section: `upstream` command behavior.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct UpstreamConfig {
    /// Verify upstream tarball signatures against
    /// `debian/upstream/signing-key.asc` when it exists.
    pub signature_check: bool,
    /// Strip the `Files-Excluded` paths from upstream tarballs.
    pub repack: bool,
}

impl Default for UpstreamConfig {
    fn default() -> Self {
        Self {
            signature_check: true,
            repack: true,
        }
    }
}

/// `[sign]` section: whether and how to sign the build artifacts.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct SignConfig {
    /// Sign the source package (`.changes`/`.dsc`) after building: `no`
    /// never, `auto` skips files our key already signed, `force` always
    /// re-signs.
    pub source: SignMode,
    /// GPG key ID/email to sign with. `None` falls back to the
    /// `Changed-By:`/`Maintainer:` address of the file being signed.
    pub key: Option<String>,
    /// Which OpenPGP implementation to use.
    pub tool: SignTool,
    /// Custom signing command when `tool` is `custom`, like debsign's `-p`.
    pub sign_command: Option<String>,
    /// Custom verification command when `tool` is `custom`, used for
    /// upstream tarball signature checks; falls back to `sign_command`.
    pub verify_command: Option<String>,
    /// Custom same-key check when `tool` is `custom`, for `source =
    /// "auto"`: exit code 0 means the file's signature is ours and it
    /// is skipped, anything else means re-sign.
    pub signed_by_command: Option<String>,
    /// Custom key fingerprint reader when `tool` is `custom`, for PPA
    /// signing keys: prints the primary key's fingerprint of `{file}`.
    pub fingerprint_command: Option<String>,
    /// Send a desktop notification via `notify-send` just before signing,
    /// so a hardware-key touch prompt isn't missed; the top-level `notify`
    /// when unset.
    pub notify: Option<bool>,
}

impl SignConfig {
    /// Whether signing is set up explicitly: a key, or a custom command
    /// that brings its own. Without either the key is guessed from the
    /// `Changed-By:`/`Maintainer:` address, which is fine when asked for
    /// with `--sign`, but not for signing by default.
    pub fn is_configured(&self) -> bool {
        self.key.is_some() || (self.tool == SignTool::Custom && self.sign_command.is_some())
    }
}

const MOVED_KEYS: &[(&str, &str)] = &[
    ("driver.apt_mirror", "apt.mirror"),
    ("driver.apt_update_age", "apt.update_age"),
    ("driver.proposed", "apt.proposed"),
];

impl Default for Config {
    fn default() -> Self {
        Self {
            driver: DriverConfig::default(),
            apt: AptConfig::default(),
            environments_dir: crate::data_dir::default_environments_dir(),
            output_dir: PathBuf::from("build-area"),
            output_dir_fallback: PathBuf::from("build-debmagic"),
            output_dir_clash: OutputDirClash::default(),
            incremental: false,
            source_sync_mode: SourceSyncMode::default(),
            build_debug_symbols: false,
            run_test: true,
            sign: SignConfig::default(),
            upload: UploadConfig::default(),
            orig_tarball: OrigTarballConfig::default(),
            upstream: UpstreamConfig::default(),
            clean: false,
            notify: false,
            build: BuildConfig::default(),
            test: TestConfig::default(),
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
        // unknown keys are ignored, which would silently drop these
        for (old, new) in MOVED_KEYS {
            if build.get::<config::Value>(old).is_ok() {
                bail!("config key '{old}' is now '{new}'");
            }
        }
        let mut config: Self = build
            .try_deserialize()
            .map_err(|e| anyhow!("Failed to read config: {e}"))?;
        config.environments_dir = resolve_environments_dir(&config.environments_dir)?;
        Ok(config)
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

/// Host roots are stored in the Environment registry and used from any
/// working directory later, so `environments_dir` must not depend on the cwd.
/// Config layers are merged before this point, so there is no single file to
/// resolve a relative value against: only `~/` is expanded.
fn resolve_environments_dir(dir: &Path) -> anyhow::Result<PathBuf> {
    if let Ok(rest) = dir.strip_prefix("~") {
        let home = dirs::home_dir().context("cannot expand ~ in environments_dir")?;
        return Ok(home.join(rest));
    }
    if !dir.is_absolute() {
        bail!(
            "environments_dir must be an absolute path (or start with ~/), got {}",
            dir.display()
        );
    }
    Ok(dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::DriverType;

    #[test]
    fn environments_dir_must_not_depend_on_the_cwd() -> anyhow::Result<()> {
        assert_eq!(
            resolve_environments_dir(Path::new("/var/debmagic"))?,
            PathBuf::from("/var/debmagic")
        );
        let home = dirs::home_dir().unwrap();
        assert_eq!(
            resolve_environments_dir(Path::new("~/envs"))?,
            home.join("envs")
        );
        assert!(resolve_environments_dir(Path::new("envs")).is_err());
        Ok(())
    }

    #[test]
    fn set_targets_the_project_config_unless_global() -> anyhow::Result<()> {
        let source_dir = Path::new("/src/pkg");
        let target = resolve_set_target(Some(source_dir), None, false)?;
        assert_eq!(target.layer, ConfigLayer::Project);
        assert_eq!(target.path, source_dir.join("debian/debmagic.toml"));
        let target = resolve_set_target(Some(source_dir), None, true)?;
        assert_eq!(target.layer, ConfigLayer::Global);
        Ok(())
    }

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
        assert_eq!(cfg.driver.persistent, crate::driver::Persistence::Always);

        assert!(
            cfg.driver.docker.base_images.get("debian:trixie")
                == Some(&"some-debian-trixie-image:latest".to_string())
        );

        Ok(())
    }

    #[test]
    fn it_loads_the_sign_section() -> Result<(), anyhow::Error> {
        let _dir_tmp = tempfile::Builder::new()
            .prefix("debmagic-test-")
            .tempdir()
            .unwrap();
        let dir = _dir_tmp.path().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let file = dir.join("sign.toml");
        std::fs::write(
            &file,
            "[sign]\nsource = \"auto\"\nkey = \"you@example.com\"\nsign_command = \"gpg --foo\"\nnotify = true\n",
        )?;
        let cfg = Config::new(&[ConfigPath::new(
            ConfigLayer::Explicit,
            file.clone(),
            ConfigPathStatus::Used,
        )])?;
        assert_eq!(cfg.sign.source, crate::sign::SignMode::Auto);
        assert_eq!(cfg.sign.key.as_deref(), Some("you@example.com"));
        assert_eq!(cfg.sign.sign_command.as_deref(), Some("gpg --foo"));
        assert_eq!(cfg.sign.notify, Some(true));
        Ok(())
    }

    #[test]
    fn it_loads_the_apt_section_and_rejects_moved_keys() -> Result<(), anyhow::Error> {
        let _dir_tmp = tempfile::Builder::new()
            .prefix("debmagic-apt-")
            .tempdir()
            .unwrap();
        let dir = _dir_tmp.path().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let load = |content: &str| {
            let file = dir.join("apt.toml");
            std::fs::write(&file, content)?;
            Config::new(&[ConfigPath::new(
                ConfigLayer::Explicit,
                file,
                ConfigPathStatus::Used,
            )])
        };
        let cfg = load(
            "[apt]\nmirror = \"http://m/ubuntu\"\n\
             [apt.packages]\n\"src:foo\" = { from = \"proposed\", version = \"1.0\" }\n",
        )?;
        assert_eq!(cfg.apt.mirror.as_deref(), Some("http://m/ubuntu"));
        let settings = &cfg.apt.packages["src:foo"];
        assert_eq!(settings.from.as_deref(), Some("proposed"));
        assert_eq!(settings.version.as_deref(), Some("1.0"));
        let error = load("[driver]\nproposed = true\n").unwrap_err();
        assert!(error.to_string().contains("'apt.proposed'"), "{error}");
        Ok(())
    }

    #[test]
    fn build_sign_defaults_survive_a_partial_section() -> Result<(), anyhow::Error> {
        let defaults = Config::default();
        assert!(defaults.build.sign_source && !defaults.build.sign_binary);

        let _dir_tmp = tempfile::Builder::new()
            .prefix("debmagic-test-build-sign-")
            .tempdir()
            .unwrap();
        let dir = _dir_tmp.path().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let file = dir.join("build.toml");
        std::fs::write(&file, "[build]\nsign_binary = true\n")?;
        let cfg = Config::new(&[ConfigPath::new(
            ConfigLayer::Explicit,
            file,
            ConfigPathStatus::Used,
        )])?;
        assert!(cfg.build.sign_source && cfg.build.sign_binary);
        Ok(())
    }

    #[test]
    fn sign_is_configured_by_a_key_or_a_custom_command() {
        let mut sign = SignConfig::default();
        assert!(!sign.is_configured());

        sign.sign_command = Some("my-signer {file}".into());
        assert!(!sign.is_configured(), "a command is ignored by gpg");
        sign.tool = SignTool::Custom;
        assert!(sign.is_configured());

        let with_key = SignConfig {
            key: Some("you@example.com".into()),
            ..SignConfig::default()
        };
        assert!(with_key.is_configured());
    }

    #[test]
    fn notify_defaults_each_step() {
        let mut cfg = Config {
            notify: true,
            ..Config::default()
        };
        cfg.sign.notify = Some(false);
        assert!(cfg.build_notify());
        assert!(cfg.test_notify());
        assert!(!cfg.sign_notify());

        cfg.override_notify(None);
        assert!(!cfg.sign_notify());
        cfg.override_notify(Some(false));
        assert!(!cfg.build_notify() && !cfg.test_notify() && !cfg.sign_notify());
        cfg.override_notify(Some(true));
        assert!(cfg.build_notify() && cfg.test_notify() && cfg.sign_notify());
    }
}
