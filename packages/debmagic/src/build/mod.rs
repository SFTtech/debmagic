use std::sync::{Arc, Mutex};
use std::{
    fs, io,
    io::{BufReader, IsTerminal, stdout},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::build::attach::{send_socket_command, start_socket_server};
use crate::build::source::{source_manifest_path, stage_source_tree};
use crate::build_intent::BuildIntent;
use crate::driver::{
    Driver, DriverInstance, DriverType, Environment, EnvironmentMetadata, EnvironmentPurpose,
    config::DriverConfig, create_driver, create_driver_from_metadata, remove_environment_root,
};
use crate::{
    config::Config,
    package::{PackageIdentity, PackageTarget},
    signing::{self, SignWith},
};
use anyhow::{Context, anyhow};

pub mod artifacts;
pub mod attach;
pub mod source;

pub use source::SourceSyncMode;

struct Build {
    environment: Environment,
    driver: DriverInstance,
    /// Prepared when signing happens inside a container: agent socket +
    /// sign key, validated before the build starts.
    gpg_forwarding: Option<signing::GpgForwarding>,
    attached: bool,
    output_dir: PathBuf,
    sign_package: bool,
    clean: bool,
    build_debug_symbols: bool,
    host_arch_variant: Option<String>,
}

/// Where debsign will actually run for this build.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SignLocation {
    Host,
    Container,
}

/// Resolve the effective sign location and validate everything signing will
/// need, so a broken gpg setup doesn't waste a whole build. Must run before
/// any environment is created — the checks are host-side and free, while
/// bootstrapping a container is not.
fn prepare_signing(
    driver: DriverType,
    sign_with: SignWith,
    sign_key: Option<&str>,
) -> anyhow::Result<(SignLocation, Option<signing::GpgForwarding>)> {
    let container_driver = driver != DriverType::Bare;
    let host_has_debsign = signing::check_host_debsign_available().is_ok();

    let location = match sign_with {
        SignWith::Host => SignLocation::Host,
        SignWith::Same => {
            if container_driver {
                SignLocation::Container
            } else {
                // The bare driver's "build environment" is the host.
                SignLocation::Host
            }
        }
        SignWith::Auto => {
            if host_has_debsign || !container_driver {
                SignLocation::Host
            } else {
                SignLocation::Container
            }
        }
    };

    match location {
        SignLocation::Host => {
            signing::check_host_debsign_available()?;
            Ok((location, None))
        }
        SignLocation::Container => {
            let sign_key = sign_key.ok_or_else(|| {
                anyhow!(
                    "signing in a container requires sign_key to be set \
                     (debsign's maintainer-based key lookup only works on the host)"
                )
            })?;
            let forwarding = signing::GpgForwarding {
                agent_extra_socket: signing::gpg_agent_extra_socket()?,
                sign_key: sign_key.to_string(),
            };
            signing::check_signing_key_available(sign_key)?;
            Ok((location, Some(forwarding)))
        }
    }
}

impl Build {
    pub fn create(
        environment: Environment,
        intent: &BuildIntent,
        gpg_forwarding: Option<signing::GpgForwarding>,
    ) -> anyhow::Result<Self> {
        let driver = create_driver(
            &environment,
            &intent.config.driver,
            &intent.driver_overrides,
        )
        .context(format!("failed to create {:?} driver", environment.driver))?;
        Ok(Self {
            environment,
            driver,
            gpg_forwarding,
            attached: false,
            output_dir: intent.output_dir.clone(),
            sign_package: intent.config.sign_package,
            clean: intent.config.clean,
            build_debug_symbols: intent.config.build_debug_symbols,
            host_arch_variant: intent.config.host_arch_variant.clone(),
        })
    }

    pub fn from_build_root(
        build_root: &Path,
        driver_config: &DriverConfig,
    ) -> anyhow::Result<Self> {
        let metadata_path = build_root.join("environment.json");
        if !metadata_path.is_file() {
            return Err(anyhow!("No environment.json found"));
        }
        let file = fs::OpenOptions::new().read(true).open(&metadata_path)?;
        let metadata = || -> anyhow::Result<EnvironmentMetadata> {
            let reader = BufReader::new(&file);
            let metadata: EnvironmentMetadata =
                serde_json::from_reader(reader).with_context(|| {
                    format!(
                        "Failed to read environment metadata from {} - invalid json",
                        metadata_path.display()
                    )
                })?;
            Ok(metadata)
        }();

        let metadata = metadata?;

        let driver = create_driver_from_metadata(driver_config, &metadata)?;

        let attached = send_socket_command(build_root, "attach").is_ok();

        Ok(Self {
            gpg_forwarding: None,
            environment: metadata.environment.clone(),
            driver,
            attached,
            output_dir: PathBuf::new(),
            sign_package: false,
            clean: false,
            build_debug_symbols: false,
            host_arch_variant: None,
        })
    }

    pub fn detach(&self) -> anyhow::Result<()> {
        let build_root = &self.environment.root_dir;
        if self.attached {
            send_socket_command(build_root, "detach")?;
        }
        Ok(())
    }

    pub fn write_metadata(&self) -> anyhow::Result<()> {
        let metadata = EnvironmentMetadata {
            environment: self.environment.clone(),
            driver_metadata: self.driver.driver_metadata(),
        };
        let path = self.environment.root_dir.join("environment.json");
        let json = serde_json::to_string_pretty(&metadata)
            .context("Failed to serialize environment metadata")?;
        fs::write(path, json)?;
        Ok(())
    }
}

fn get_build_root_and_identifier(
    temp_build_dir: &Path,
    identity: &PackageIdentity,
) -> (String, PathBuf) {
    let package_identifier = format!("{}-{}", identity.name, identity.version);
    let build_root = temp_build_dir.join(&package_identifier);
    (package_identifier, build_root)
}

fn prepare_build_env(
    intent: &BuildIntent,
    target: &PackageTarget,
    gpg_forwarding: Option<signing::GpgForwarding>,
) -> anyhow::Result<Build> {
    let (package_identifier, build_root) =
        get_build_root_and_identifier(&intent.config.temp_build_dir, &target.identity);

    let environment = Environment {
        driver: intent.driver,
        package_name: target.identity.name.clone(),
        package_identifier,
        root_dir: build_root.clone(),
        distro: target.distro.clone(),
        persistent: intent.config.driver.persistent,
        purpose: EnvironmentPurpose::Build,
    };

    let output_dir = &intent.output_dir;
    let incremental = intent.config.incremental;

    if intent.config.driver.persistent && build_root.exists() {
        // For persistent containers, starting first lets root inside delete
        // container-owned files the host user can't remove.
        let build = Build::create(environment.clone(), intent, gpg_forwarding)
            .context(format!("failed to create {:?} driver", intent.driver))?;
        if !incremental || !source_manifest_path(&environment).is_file() {
            build
                .driver
                .reset_root()
                .context("failed to reset persistent build directory")?;
        } else if !build.driver.reused_environment() {
            // A fresh environment (e.g. a new CI runner with a restored build
            // tree) keeps incremental outputs; cargo's own fingerprinting
            // discards whatever the new toolchain/archive state invalidates.
            println!("Keeping incremental build tree in a fresh build environment");
        }
        fs::create_dir_all(output_dir).context("failed to create output directory")?;
        environment
            .create_dirs()
            .context("failed to create build directories")?;
        stage_source_tree(
            &environment,
            &target.identity,
            intent.config.source_sync_mode,
            incremental,
        )?;
        return Ok(build);
    }

    remove_environment_root(&build_root, &intent.config.driver)?;

    fs::create_dir_all(output_dir).context("failed to create output directory")?;
    environment
        .create_dirs()
        .context("failed to create build directories")?;

    stage_source_tree(
        &environment,
        &target.identity,
        intent.config.source_sync_mode,
        incremental,
    )?;

    Build::create(environment, intent, gpg_forwarding)
}

pub fn get_shell_in_build(config: &Config, identity: &PackageIdentity) -> anyhow::Result<()> {
    let (_package_identifier, build_root) =
        get_build_root_and_identifier(&config.temp_build_dir, identity);
    let build = Build::from_build_root(&build_root, &config.driver)?;
    let result = build
        .driver
        .interactive_shell(&build.environment.staged_source_dir());

    build.detach()?;

    result?;
    Ok(())
}

fn deb_build_options(existing: Option<&str>, build_debug_symbols: bool) -> String {
    let mut options = existing
        .unwrap_or_default()
        .split_whitespace()
        .filter(|option| *option != "noautodbgsym")
        .collect::<Vec<_>>();
    if !build_debug_symbols {
        options.push("noautodbgsym");
    }
    options.join(" ")
}

/// Everything needed to run one package build, independent of whether the
/// build produces binary or source packages.
struct BuildRequest<'a> {
    intent: &'a BuildIntent,
    target: &'a PackageTarget,
}

/// Shared build orchestration: prepare the environment, run `build_commands`
/// in it, export the artifacts to the output dir, sign them if requested, and
/// clean up (dropping into a shell first when `--shell-on-failure` is set).
/// While the run is in progress, a socket server lets concurrent
/// `debmagic shell` sessions attach to the environment.
fn run_build(
    request: &BuildRequest,
    build_commands: impl FnOnce(&Build) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let gpg_forwarding = if request.intent.config.sign_package {
        let (_location, forwarding) = prepare_signing(
            request.intent.driver,
            request.intent.config.sign_with,
            request.intent.config.sign_key.as_deref(),
        )?;
        forwarding
    } else {
        None
    };
    let build = prepare_build_env(request.intent, request.target, gpg_forwarding)
        .context("failed to prepare build environment")?;
    build
        .write_metadata()
        .context("failed to write environment metadata")?;

    let should_exit = Arc::new(Mutex::new(false));
    let socket_server_handle =
        start_socket_server(&build.environment.root_dir, should_exit.clone())?;

    let stop_socket_server = || {
        *should_exit.lock().unwrap() = true;
        if !socket_server_handle.is_finished() {
            println!("Waiting for all attached shells to exit...");
        }
        socket_server_handle.join().ok();
    };

    let sign_key = request.intent.config.sign_key.as_deref();
    let result = build_commands(&build).and_then(|()| {
        let changes_file =
            artifacts::export_build_artifacts(&build.environment.work_dir(), &build.output_dir)?;
        if build.sign_package {
            build
                .driver
                .sign_changes(&changes_file, build.gpg_forwarding.as_ref(), sign_key)?;
        }
        Ok(())
    });

    if let Err(error) = result {
        if request.intent.shell_on_failure && stdout().is_terminal() {
            eprintln!("Build failed: {error}. Dropping into shell...");
            if let Err(shell_error) = build
                .driver
                .interactive_shell(&build.environment.staged_source_dir())
            {
                eprintln!("Dropping into shell failed: {shell_error}");
            }
        } else if request.intent.shell_on_failure {
            eprintln!("Build failed: {error}");
            eprintln!(
                "--shell-on-failure is set but stdout is not a TTY; skipping interactive shell"
            );
        } else {
            eprintln!("Build failed: {error}");
            eprintln!("Re-run with --shell-on-failure to inspect the build environment");
        }
        if let Err(cleanup_error) = build.driver.cleanup() {
            eprintln!("Failed to clean up build environment: {cleanup_error}");
        }
        stop_socket_server();
        return Err(error);
    }

    stop_socket_server();
    build
        .driver
        .cleanup()
        .context("failed to clean up build environment")?;
    Ok(())
}

pub fn build_package(intent: &BuildIntent, target: &PackageTarget) -> anyhow::Result<()> {
    let request = BuildRequest { intent, target };
    run_build(&request, |build| {
        build.driver.run_command_checked(
            &["apt-get", "-y", "build-dep", "."],
            &build.environment.staged_source_dir(),
            true,
            &[],
        )?;
        let inherited_options = std::env::var("DEB_BUILD_OPTIONS").ok();
        let options = deb_build_options(inherited_options.as_deref(), build.build_debug_symbols);
        let mut env_add = vec![("DEB_BUILD_OPTIONS", options.as_str())];
        if let Some(variant) = build.host_arch_variant.as_deref() {
            env_add.push(("DEB_HOST_ARCH_VARIANT", variant));
        }
        let mut dpkg_buildpackage_args = vec!["dpkg-buildpackage", "-us", "-uc", "-ui"];
        if !build.clean {
            // Non-incremental builds already stage a clean source tree, while
            // incremental builds preserve their outputs intentionally.
            dpkg_buildpackage_args.push("-nc");
        }
        dpkg_buildpackage_args.push("-b");
        build.driver.run_command_checked(
            &dpkg_buildpackage_args,
            &build.environment.staged_source_dir(),
            false,
            &env_add,
        )?;
        Ok(())
    })
}

/// Confirm `cmd` is on `PATH`, failing with an actionable message (rather
/// than a raw "command not found") if it isn't.
fn check_command_available(cmd: &str, install_hint: &str) -> anyhow::Result<()> {
    match Command::new(cmd)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err(anyhow!("{cmd} not found on PATH. {install_hint}"))
        }
        Err(e) => Err(e).with_context(|| format!("failed to check for {cmd}")),
    }
}

fn check_dpkg_buildpackage_available() -> anyhow::Result<()> {
    check_command_available(
        "dpkg-buildpackage",
        "It's part of dpkg-dev; install it, or pass --driver lxd/incus/docker to build inside a Debian-ish container instead.",
    )
}

/// Build a `.dsc` + tarball + `.buildinfo` + `.changes` source package.
///
/// If `config.clean` is set, build-dependencies are installed before
/// `dpkg-buildpackage` runs `debian/rules clean` once.
pub fn build_source_package(intent: &BuildIntent, target: &PackageTarget) -> anyhow::Result<()> {
    if intent.driver == DriverType::Bare {
        check_dpkg_buildpackage_available()?;
    }

    let request = BuildRequest { intent, target };
    run_build(&request, |build| {
        let staged_source_dir = build.environment.staged_source_dir();
        if build.clean {
            build.driver.run_command_checked(
                &["apt-get", "-y", "build-dep", "."],
                &staged_source_dir,
                true,
                &[],
            )?;
        }
        let mut args = vec!["dpkg-buildpackage", "-S", "-d", "-us", "-uc", "-ui"];
        if !build.clean {
            args.push("-nc");
        }
        build
            .driver
            .run_command_checked(&args, &staged_source_dir, false, &[])?;
        Ok(())
    })
    .context("failed to build source package")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_symbol_option_preserves_other_build_options() {
        assert_eq!(
            deb_build_options(Some("nocheck parallel=8"), false),
            "nocheck parallel=8 noautodbgsym"
        );
        assert_eq!(
            deb_build_options(Some("nocheck noautodbgsym parallel=8"), true),
            "nocheck parallel=8"
        );
    }
}
