use std::path::PathBuf;

use anyhow::Context;

use crate::{
    build::source::SourceSyncMode,
    config::Config,
    driver::{DriverType, Persistence, config::DriverOverrides},
    sign::{SignMode, SignOverrides},
};

/// Which kind of package a build produces: a source-only upload or
/// binaries. Decides the dpkg-buildpackage mode and which `.changes` set a
/// (possibly persistent) work dir exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildKind {
    Source,
    Binary,
}

impl BuildKind {
    pub fn is_source(self) -> bool {
        self == Self::Source
    }
}

/// Inputs for resolving a [`BuildIntent`].
#[derive(Debug, Clone)]
pub struct BuildIntentInput {
    pub source_dir: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub driver: Option<DriverType>,
    pub kind: BuildKind,
    pub persistent: Option<Persistence>,
    pub incremental: Option<bool>,
    pub debug_symbols: Option<bool>,
    pub test: Option<bool>,
    pub sign: Option<SignMode>,
    pub sign_overrides: SignOverrides,
    pub notify: Option<bool>,
    pub clean: Option<bool>,
    pub source_sync: Option<SourceSyncMode>,
    pub host_arch_variant: Option<String>,
    pub driver_overrides: DriverOverrides,
}

/// Fully resolved description of *how* a package build should run.
///
/// Does not include *what* is being built (package identity or target distro).
#[derive(Debug, Clone)]
pub struct BuildIntent {
    pub source_dir: PathBuf,
    pub output_dir: PathBuf,
    pub driver: DriverType,
    pub kind: BuildKind,
    pub config: Config,
    pub driver_overrides: DriverOverrides,
}

pub fn resolve_build_intent(input: BuildIntentInput) -> anyhow::Result<BuildIntent> {
    // Canonicalized: Environment ids and registry lookups key on the
    // source_dir string, so all entry points must agree on one spelling.
    let source_dir =
        std::fs::canonicalize(input.source_dir).context("resolving source dir failed")?;

    let mut config = Config::load(Some(&source_dir), input.config_file.as_deref())?;

    let driver = resolve_driver(input.driver, config.driver.default, input.kind)?;

    let output_dir = crate::output_dir::resolve(&source_dir, &config, input.output_dir.as_deref())?;

    if let Some(persistent) = input.persistent {
        config.driver.persistent = persistent;
    }

    if let Some(incremental) = input.incremental {
        config.incremental = incremental;
    }

    if let Some(debug_symbols) = input.debug_symbols {
        config.build_debug_symbols = debug_symbols;
    }
    if let Some(test) = input.test {
        config.run_test = test;
    }
    input.sign_overrides.apply(&mut config.sign);
    config.sign.source = resolve_sign_mode(input.sign, &config, input.kind);
    config.override_notify(input.notify);
    if let Some(clean) = input.clean {
        config.clean = clean;
    }
    if let Some(source_sync) = input.source_sync {
        config.source_sync_mode = source_sync;
    }
    if let Some(host_arch_variant) = input.host_arch_variant {
        config.host_arch_variant = Some(host_arch_variant);
    }
    if config.incremental {
        if config.clean {
            anyhow::bail!("incremental builds are incompatible with clean builds");
        }
        config.driver.persistent = Persistence::Always;
    }

    Ok(BuildIntent {
        source_dir,
        output_dir,
        driver,
        kind: input.kind,
        config,
        driver_overrides: input.driver_overrides,
    })
}

/// How to sign a build of `kind`. `--sign` decides when given (so it signs
/// even without a configured key); otherwise `build.sign_source` or
/// `build.sign_binary` does, and only when a signing key is configured,
/// since the key would otherwise be guessed from the maintainer address.
/// The mode then comes from `sign.source`, where `no` still disables.
fn resolve_sign_mode(cli: Option<SignMode>, config: &Config, kind: BuildKind) -> SignMode {
    if let Some(mode) = cli {
        return mode;
    }
    let by_default = match kind {
        BuildKind::Source => config.build.sign_source,
        BuildKind::Binary => config.build.sign_binary,
    };
    if by_default && config.sign.is_configured() {
        config.sign.source
    } else {
        SignMode::No
    }
}

/// CLI `--driver` wins; else the config's `driver.default`; source-only
/// builds fall back to bare, since they need no build-deps or compilation.
fn resolve_driver(
    cli: Option<DriverType>,
    config_default: Option<DriverType>,
    kind: BuildKind,
) -> anyhow::Result<DriverType> {
    match cli.or(config_default) {
        Some(driver) => Ok(driver),
        None if kind.is_source() => Ok(DriverType::Bare),
        None => anyhow::bail!(
            "no build driver selected: pass --driver or set 'driver' in debmagic.toml (docker, bare, lxd or incus)"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{
        DriverType, Persistence, config::DriverOverrides, driver_bare::DriverBareConfigOverrides,
        driver_docker::DriverDockerConfigOverrides, driver_lxd::DriverLxdConfigOverrides,
    };
    use test_case::test_case;

    fn asset_config() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets")
            .join("config1.toml")
    }

    fn base_input(source_dir: PathBuf) -> BuildIntentInput {
        BuildIntentInput {
            source_dir,
            output_dir: None,
            config_file: Some(asset_config()),
            driver: Some(DriverType::Docker),
            kind: BuildKind::Binary,
            persistent: None,
            incremental: None,
            debug_symbols: None,
            test: None,
            sign: None,
            sign_overrides: SignOverrides::default(),
            notify: None,
            clean: None,
            source_sync: None,
            host_arch_variant: None,
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
    fn load_config_reads_explicit_file() -> anyhow::Result<()> {
        let cfg = Config::load(None, Some(&asset_config()))?;
        assert_eq!(cfg.driver.persistent, Persistence::Always);
        assert_eq!(
            cfg.driver.docker.base_images.get("debian:trixie"),
            Some(&"some-debian-trixie-image:latest".to_string())
        );
        Ok(())
    }

    #[test]
    fn resolve_defaults_driver_from_config() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        input.driver = None;

        let intent = resolve_build_intent(input)?;
        // config1.toml sets driver.default = "docker"
        assert_eq!(intent.driver, DriverType::Docker);
        Ok(())
    }

    #[test]
    fn resolve_driver_without_any_default() {
        // source-only builds fall back to bare
        assert_eq!(
            resolve_driver(None, None, BuildKind::Source).unwrap(),
            DriverType::Bare
        );

        // binary builds require a driver
        let result = resolve_driver(None, None, BuildKind::Binary);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no build driver selected")
        );
    }

    #[test]
    fn resolve_driver_precedence() -> anyhow::Result<()> {
        // CLI wins over the config default
        assert_eq!(
            resolve_driver(
                Some(DriverType::Lxd),
                Some(DriverType::Docker),
                BuildKind::Binary
            )?,
            DriverType::Lxd
        );
        // config default when no CLI driver
        assert_eq!(
            resolve_driver(None, Some(DriverType::Docker), BuildKind::Binary)?,
            DriverType::Docker
        );
        Ok(())
    }

    #[test]
    fn resolve_applies_incremental_implies_persistent() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        input.persistent = Some(Persistence::No);
        input.incremental = Some(true);

        let intent = resolve_build_intent(input)?;
        assert!(intent.config.incremental);
        assert_eq!(intent.config.driver.persistent, Persistence::Always);
        Ok(())
    }

    #[test]
    fn resolve_honours_persistent_without_incremental() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        // config1.toml has persistent = "always"; CLI can turn it off
        input.persistent = Some(Persistence::No);
        input.incremental = Some(false);

        let intent = resolve_build_intent(input)?;
        assert!(!intent.config.incremental);
        assert_eq!(intent.config.driver.persistent, Persistence::No);
        Ok(())
    }

    #[test]
    fn leftover_shell_on_failure_key_is_ignored() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let mut input = base_input(dir);
        let config_path = std::env::temp_dir().join(format!(
            "debmagic-shell-on-failure-{}.toml",
            std::process::id()
        ));
        std::fs::write(&config_path, "shell_on_failure = true\n")?;
        input.config_file = Some(config_path.clone());

        let intent = resolve_build_intent(input)?;
        assert_eq!(intent.config.driver.persistent, Persistence::OnFailure);
        std::fs::remove_file(config_path)?;
        Ok(())
    }

    #[test]
    fn resolve_absolutizes_paths() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let intent = resolve_build_intent(base_input(dir.clone()))?;
        assert!(intent.source_dir.is_absolute());
        assert!(intent.output_dir.is_absolute());
        assert_eq!(intent.source_dir, std::fs::canonicalize(&dir)?);
        // default output dir is build/, relative to the package root
        assert_eq!(
            intent.output_dir,
            std::path::absolute(std::fs::canonicalize(&dir)?.join("build"))?
        );
        Ok(())
    }

    fn sign_test_config(key: bool, sign_source: bool, sign_binary: bool) -> Config {
        let mut config = Config::default();
        config.sign.key = key.then(|| "you@example.com".to_string());
        config.build.sign_source = sign_source;
        config.build.sign_binary = sign_binary;
        config
    }

    #[test_case(BuildKind::Source, None, true, true, false, SignMode::Auto; "source builds sign by default")]
    #[test_case(BuildKind::Binary, None, true, true, false, SignMode::No; "binary builds do not")]
    #[test_case(BuildKind::Binary, Some(SignMode::Auto), true, true, false, SignMode::Auto; "--sign signs a binary build")]
    #[test_case(BuildKind::Binary, Some(SignMode::Auto), false, true, false, SignMode::Auto; "--sign needs no configured key")]
    #[test_case(BuildKind::Binary, Some(SignMode::Force), true, true, false, SignMode::Force; "--sign=force")]
    #[test_case(BuildKind::Source, Some(SignMode::No), true, true, false, SignMode::No; "--sign=no")]
    #[test_case(BuildKind::Source, None, false, true, false, SignMode::No; "no key, no default signing")]
    #[test_case(BuildKind::Binary, None, true, true, true, SignMode::Auto; "build.sign_binary")]
    #[test_case(BuildKind::Source, None, true, false, false, SignMode::No; "build.sign_source = false")]
    fn resolve_sign_mode_by_kind(
        kind: BuildKind,
        cli: Option<SignMode>,
        key: bool,
        sign_source: bool,
        sign_binary: bool,
        expected: SignMode,
    ) {
        let config = sign_test_config(key, sign_source, sign_binary);
        assert_eq!(resolve_sign_mode(cli, &config, kind), expected);
    }

    #[test]
    fn resolve_sign_mode_uses_the_configured_mode() {
        let mut config = sign_test_config(true, true, true);
        config.sign.source = SignMode::Force;
        assert_eq!(
            resolve_sign_mode(None, &config, BuildKind::Binary),
            SignMode::Force
        );

        config.sign.source = SignMode::No;
        assert_eq!(
            resolve_sign_mode(None, &config, BuildKind::Source),
            SignMode::No
        );
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
}
