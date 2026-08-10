use std::sync::{Arc, Mutex};
use std::{
    fs, io,
    io::{BufReader, IsTerminal, stdout},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::build::attach::{send_socket_command, start_socket_server};
use crate::build::config::DriverOverrides;
use crate::build::source::{source_manifest_path, stage_source_tree};
use crate::build_intent::BuildIntent;
use crate::{
    build::{
        common::{BuildConfig, BuildDriver, BuildDriverType, BuildMetadata},
        config::DriverConfig,
        driver_bare::DriverBare,
        driver_docker::DriverDocker,
        driver_lxd::{DriverLxd, LxdVariant},
    },
    config::Config,
    package::{PackageIdentity, PackageTarget},
};
use anyhow::{Context, anyhow};

pub mod artifacts;
pub mod attach;
pub mod common;
pub mod config;
pub mod driver_bare;
pub mod driver_docker;
pub mod driver_lxd;
pub mod signing;
pub mod source;

struct Build {
    config: BuildConfig,
    pub driver: Box<dyn BuildDriver>,
    /// Prepared when signing happens inside a container: agent socket +
    /// sign key, validated before the build starts.
    gpg_forwarding: Option<signing::GpgForwarding>,
    attached: bool,
}

/// Where debsign will actually run for this build.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SignLocation {
    Host,
    Container,
}

/// Resolve the effective sign location and validate everything signing will
/// need *before* the build starts, so a broken gpg setup doesn't waste a
/// whole build.
fn prepare_signing(
    build_config: &BuildConfig,
) -> anyhow::Result<(SignLocation, Option<signing::GpgForwarding>)> {
    let container_driver = build_config.driver != BuildDriverType::Bare;
    let host_has_debsign = signing::check_host_debsign_available().is_ok();

    let location = match build_config.sign_with {
        signing::SignWith::Host => SignLocation::Host,
        signing::SignWith::Same => {
            if container_driver {
                SignLocation::Container
            } else {
                // The bare driver's "build environment" is the host.
                SignLocation::Host
            }
        }
        signing::SignWith::Auto => {
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
            let sign_key = build_config.sign_key.clone().ok_or_else(|| {
                anyhow!(
                    "signing in a container requires sign_key to be set \
                     (debsign's maintainer-based key lookup only works on the host)"
                )
            })?;
            let forwarding = signing::GpgForwarding {
                agent_extra_socket: signing::gpg_agent_extra_socket()?,
                sign_key: sign_key.clone(),
            };
            signing::check_signing_key_available(&sign_key)?;
            Ok((location, Some(forwarding)))
        }
    }
}

fn get_build_driver(
    config: &BuildConfig,
    driver_config: &DriverConfig,
    driver_overrides: &DriverOverrides,
) -> anyhow::Result<Box<dyn BuildDriver>> {
    let apt_mirror = driver_overrides
        .apt_mirror
        .as_deref()
        .or(driver_config.apt_mirror.as_deref());
    let proposed = driver_overrides.proposed.unwrap_or(driver_config.proposed);

    match config.driver {
        BuildDriverType::Docker => Ok(Box::new(DriverDocker::create(
            config,
            driver_config,
            &driver_overrides.docker,
            apt_mirror,
            proposed,
        )?)),
        BuildDriverType::Bare => Ok(Box::new(DriverBare::create(
            config,
            driver_config,
            &driver_overrides.bare,
        ))),
        BuildDriverType::Lxd | BuildDriverType::Incus => {
            let variant = match config.driver {
                BuildDriverType::Lxd => LxdVariant::Lxd,
                _ => LxdVariant::Incus,
            };
            Ok(Box::new(DriverLxd::create(
                variant,
                config,
                driver_config,
                &driver_overrides.lxd,
                apt_mirror,
                proposed,
            )?))
        }
    }
}

fn create_driver_from_metadata(
    config: &DriverConfig,
    metadata: &BuildMetadata,
) -> anyhow::Result<Box<dyn BuildDriver>> {
    let driver: anyhow::Result<Box<dyn BuildDriver>> = match &metadata.config.driver {
        BuildDriverType::Docker => Ok(Box::new(DriverDocker::from_build_metadata(
            &metadata.config,
            config,
            metadata,
        )?)),
        BuildDriverType::Bare => Ok(Box::new(DriverBare::from_build_metadata(
            &metadata.config,
            config,
            metadata,
        ))),
        BuildDriverType::Lxd | BuildDriverType::Incus => {
            let variant = match metadata.config.driver {
                BuildDriverType::Lxd => LxdVariant::Lxd,
                _ => LxdVariant::Incus,
            };
            Ok(Box::new(DriverLxd::from_build_metadata(
                variant,
                &metadata.config,
                config,
                metadata,
            )?))
        }
    };
    driver
}

impl Build {
    pub fn create(
        config: &BuildConfig,
        driver_config: &DriverConfig,
        driver_overrides: &DriverOverrides,
    ) -> anyhow::Result<Self> {
        let driver = get_build_driver(config, driver_config, driver_overrides)
            .context(format!("failed to create {:?} build driver", config.driver))?;
        let gpg_forwarding = if config.sign_package {
            let (_location, forwarding) = prepare_signing(config)?;
            forwarding
        } else {
            None
        };
        Ok(Self {
            config: config.clone(),
            driver,
            gpg_forwarding,
            attached: false,
        })
    }

    pub fn from_build_root(
        build_root: &Path,
        driver_config: &DriverConfig,
    ) -> anyhow::Result<Self> {
        let build_metadata_path = build_root.join("build.json");
        if !build_metadata_path.is_file() {
            return Err(anyhow!("No build.json found"));
        }
        // read metadata from file
        let file = fs::OpenOptions::new()
            .read(true)
            .open(&build_metadata_path)?;
        let metadata = || -> anyhow::Result<BuildMetadata> {
            let reader = BufReader::new(&file);
            let metadata: BuildMetadata = serde_json::from_reader(reader).with_context(|| {
                format!(
                    "Failed to read build metadata from {} - invalid json",
                    build_metadata_path.display()
                )
            })?;
            Ok(metadata)
        }();

        let metadata = metadata?;

        let driver = create_driver_from_metadata(driver_config, &metadata)?;

        let attached = send_socket_command(build_root, "attach").is_ok();

        Ok(Self {
            gpg_forwarding: None,
            config: metadata.config.clone(),
            driver,
            attached,
        })
    }

    pub fn detach(&self) -> anyhow::Result<()> {
        let build_root = &self.config.build_root_dir;
        if self.attached {
            send_socket_command(build_root, "detach")?;
        }
        Ok(())
    }

    pub fn write_metadata(&self) -> anyhow::Result<()> {
        let metadata = BuildMetadata {
            config: self.config.clone(),
            driver_metadata: self.driver.get_build_metadata(),
        };
        let path = self.config.build_root_dir.join("build.json");
        let json = serde_json::to_string_pretty(&metadata)
            .context("Failed to serialize build metadata")?;
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

fn prepare_build_env(intent: &BuildIntent, target: &PackageTarget) -> anyhow::Result<Build> {
    let (package_identifier, build_root) =
        get_build_root_and_identifier(&intent.config.temp_build_dir, &target.identity);

    let build_config = BuildConfig {
        driver: intent.driver,
        package_name: target.identity.name.clone(),
        package_identifier,
        source_dir: target.identity.source_dir.clone(),
        output_dir: intent.output_dir.clone(),
        build_root_dir: build_root.clone(),
        distro: target.distro.clone(),
        sign_package: intent.config.sign_package,
        sign_with: intent.config.sign_with,
        sign_key: intent.config.sign_key.clone(),
        build_debug_symbols: intent.config.build_debug_symbols,
        clean: intent.config.clean,
        persistent: intent.config.driver.persistent,
        incremental: intent.config.incremental,
        source_sync_mode: intent.config.source_sync_mode,
    };

    if intent.config.driver.persistent && build_root.exists() {
        // For persistent containers, starting first lets root inside delete
        // container-owned files the host user can't remove.
        let build = Build::create(
            &build_config,
            &intent.config.driver,
            &intent.driver_overrides,
        )
        .context(format!("failed to create {:?} build driver", intent.driver))?;
        if !intent.config.incremental
            || !source_manifest_path(&build_config).is_file()
            || !build.driver.reused_environment()
        {
            build
                .driver
                .reset_build_root()
                .context("failed to reset persistent build directory")?;
        }
        build_config
            .create_dirs()
            .context("failed to create build directories")?;
        stage_source_tree(&build_config, &target.identity)?;
        return Ok(build);
    }

    if build_root.exists()
        && let Err(e) = fs::remove_dir_all(&build_root)
    {
        if e.kind() == io::ErrorKind::PermissionDenied {
            // Some files were created by a privileged user inside a container
            // and can't be deleted by the host user directly. Load the previous
            // build's driver and ask it to clean up from inside.
            let metadata_path = build_root.join("build.json");
            if metadata_path.is_file()
                && let Ok(file) = fs::OpenOptions::new().read(true).open(&metadata_path)
                && let Ok(metadata) =
                    serde_json::from_reader::<_, BuildMetadata>(BufReader::new(&file))
                && let Ok(driver) = create_driver_from_metadata(&intent.config.driver, &metadata)
            {
                let _ = driver.reset_build_root();
            }
            fs::remove_dir_all(&build_root).with_context(|| {
                format!(
                    "failed to remove build root {}; try: sudo rm -rf {}",
                    build_root.display(),
                    build_root.display()
                )
            })?;
        } else {
            return Err(e.into());
        }
    }

    build_config
        .create_dirs()
        .context("failed to create build directories")?;

    stage_source_tree(&build_config, &target.identity)?;

    let build = Build::create(
        &build_config,
        &intent.config.driver,
        &intent.driver_overrides,
    )?;
    Ok(build)
}

pub fn get_shell_in_build(config: &Config, identity: &PackageIdentity) -> anyhow::Result<()> {
    let (_package_identifier, build_root) =
        get_build_root_and_identifier(&config.temp_build_dir, identity);
    let build = Build::from_build_root(&build_root, &config.driver)?;
    let result = build
        .driver
        .interactive_shell(&build.config.build_source_dir());

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
/// clean up (dropping into a shell first on failure of an interactive binary
/// build). While `shell_on_failure` is set, a socket server lets concurrent
/// `debmagic shell` sessions attach to the environment.
fn run_build(
    request: &BuildRequest,
    shell_on_failure: bool,
    build_commands: impl FnOnce(&Build) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let build = prepare_build_env(request.intent, request.target)
        .context("failed to prepare build environment")?;
    build
        .write_metadata()
        .context("failed to write build metadata")?;

    let should_exit = Arc::new(Mutex::new(false));
    let socket_server_handle =
        start_socket_server(&build.config.build_root_dir, should_exit.clone())?;

    let stop_socket_server = || {
        *should_exit.lock().unwrap() = true;
        if !socket_server_handle.is_finished() {
            println!("Waiting for all attached shells to exit...");
        }
        socket_server_handle.join().ok();
    };

    let result = build_commands(&build).and_then(|()| {
        let changes_file = artifacts::export_build_artifacts(
            &build.config.build_work_dir(),
            &build.config.output_dir,
        )?;
        if build.config.sign_package {
            build
                .driver
                .sign_changes(&changes_file, build.gpg_forwarding.as_ref())?;
        }
        Ok(())
    });

    if let Err(error) = result {
        if shell_on_failure && stdout().is_terminal() {
            eprintln!("Build failed: {error}. Dropping into shell...");
            if let Err(shell_error) = build
                .driver
                .interactive_shell(&build.config.build_source_dir())
            {
                eprintln!("Dropping into shell failed: {shell_error}");
            }
        } else {
            eprintln!("Build failed: {error}");
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
    run_build(&request, true, |build| {
        build.driver.run_command(
            &["apt-get", "-y", "build-dep", "."],
            &build.config.build_source_dir(),
            true,
        )?;
        let inherited_options = std::env::var("DEB_BUILD_OPTIONS").ok();
        let options = deb_build_options(
            inherited_options.as_deref(),
            build.config.build_debug_symbols,
        );
        let env_add = [("DEB_BUILD_OPTIONS", options.as_str())];
        let mut dpkg_buildpackage_args = vec!["dpkg-buildpackage", "-us", "-uc", "-ui"];
        if !build.config.clean {
            // Non-incremental builds already stage a clean source tree, while
            // incremental builds preserve their outputs intentionally.
            dpkg_buildpackage_args.push("-nc");
        }
        dpkg_buildpackage_args.push("-b");
        build.driver.run_command_env(
            &dpkg_buildpackage_args,
            &build.config.build_source_dir(),
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
    if intent.driver == BuildDriverType::Bare {
        check_dpkg_buildpackage_available()?;
    }

    let request = BuildRequest { intent, target };
    run_build(&request, false, |build| {
        let build_source_dir = build.config.build_source_dir();
        if build.config.clean {
            build.driver.run_command(
                &["apt-get", "-y", "build-dep", "."],
                &build_source_dir,
                true,
            )?;
        }
        let mut args = vec!["dpkg-buildpackage", "-S", "-d", "-us", "-uc", "-ui"];
        if !build.config.clean {
            args.push("-nc");
        }
        build.driver.run_command(&args, &build_source_dir, false)?;
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
