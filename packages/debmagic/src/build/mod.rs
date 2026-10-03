use std::sync::{Arc, Mutex};
use std::{
    fs, io,
    io::{BufReader, IsTerminal, stdout},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::build::attach::{send_socket_command, start_socket_server};
use crate::build::source::{source_manifest_path, stage_dir, stage_source_tree};
use crate::build_intent::{BuildIntent, BuildIntentInput, BuildKind, resolve_build_intent};
use crate::driver::{
    Driver, DriverType, Environment, EnvironmentDriver, EnvironmentMetadata, EnvironmentPurpose,
    config::DriverConfig, create_driver, create_driver_from_metadata, remove_environment_root,
};
use crate::{config::Config, package::PackageTarget};
use anyhow::{Context, anyhow};
use debmagic_common::debian::source::SourceFormat;
use debmagic_common::package::SourcePackage;

pub mod artifacts;
pub mod attach;
pub mod source;

pub use source::SourceSyncMode;

struct Build {
    environment: Environment,
    driver: Driver,
    attached: bool,
    output_dir: PathBuf,
    clean: bool,
    build_debug_symbols: bool,
    run_test: bool,
    host_arch_variant: Option<String>,
}

impl Build {
    pub fn create(environment: Environment, intent: &BuildIntent) -> anyhow::Result<Self> {
        let driver = create_driver(
            &environment,
            &intent.config.driver,
            &intent.driver_overrides,
        )
        .context(format!("failed to create {:?} driver", environment.driver))?;
        Ok(Self {
            environment,
            driver,
            attached: false,
            output_dir: intent.output_dir.clone(),
            clean: intent.config.clean,
            build_debug_symbols: intent.config.build_debug_symbols,
            run_test: intent.config.run_test,
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
            environment: metadata.environment.clone(),
            driver,
            attached,
            output_dir: PathBuf::new(),
            clean: false,
            build_debug_symbols: false,
            run_test: true,
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
    package: &SourcePackage,
) -> (String, PathBuf) {
    let package_identifier = format!("{}-{}", package.name(), package.version());
    let build_root = temp_build_dir.join(&package_identifier);
    (package_identifier, build_root)
}

/// Create the output dir and stage the source tree into the environment.
fn stage_sources(
    environment: &Environment,
    intent: &BuildIntent,
    target: &PackageTarget,
) -> anyhow::Result<()> {
    fs::create_dir_all(&intent.output_dir).context("failed to create output directory")?;
    environment
        .create_dirs()
        .context("failed to create build directories")?;
    stage_source_tree(
        environment,
        intent.kind,
        &target.package,
        intent.config.source_sync_mode,
        intent.config.incremental,
    )
}

fn prepare_build_env(intent: &BuildIntent, target: &PackageTarget) -> anyhow::Result<Build> {
    let (package_identifier, build_root) =
        get_build_root_and_identifier(&intent.config.temp_build_dir, &target.package);

    let environment = Environment {
        driver: intent.driver,
        package_name: target.package.name().to_string(),
        package_identifier,
        root_dir: build_root.clone(),
        distro: target.distro.clone(),
        persistent: intent.config.driver.persistent,
        purpose: EnvironmentPurpose::Build,
    };

    if intent.config.driver.persistent && build_root.exists() {
        // For persistent containers, starting first lets root inside delete
        // container-owned files the host user can't remove.
        let build = Build::create(environment.clone(), intent)
            .context(format!("failed to create {:?} driver", intent.driver))?;
        // An incremental sync needs a manifest from a previous build of the
        // same kind; without one the tree is reset so no stale files leak
        // into the build.
        let sync_incrementally =
            intent.config.incremental && source_manifest_path(&environment, intent.kind).is_file();
        if sync_incrementally {
            if !build.driver.reused_environment() {
                // e.g. a new CI runner with a restored build tree; cargo's own
                // fingerprinting discards whatever the new toolchain/archive
                // state invalidates
                println!("Keeping incremental build tree in a fresh build environment");
            }
            unapply_quilt_patches(&build, intent.kind, &target.package)?;
        } else if intent.kind.is_source() {
            // A source build shares the environment with the binary tree:
            // only its own stage dir is restaged, never the whole root.
            reset_stage_dir(&build, &environment, intent.kind)?;
        } else {
            build
                .driver
                .reset_root()
                .context("failed to reset persistent build directory")?;
        }
        stage_sources(&environment, intent, target)?;
        return Ok(build);
    }

    remove_environment_root(&build_root, &intent.config.driver)?;

    crate::output::step("Staging source tree");
    stage_sources(&environment, intent, target)?;

    Build::create(environment, intent)
}

/// Whether an incremental sync must first unapply quilt patches in the
/// build tree: only `3.0 (quilt)` manages patches through `.pc`, and no
/// `.pc` means none are applied.
fn needs_quilt_unapply(source_format: SourceFormat, staged_source_dir: &Path) -> bool {
    source_format == SourceFormat::Quilt && staged_source_dir.join(".pc").exists()
}

/// Unapply quilt patches a previous build or shell session left in the
/// build tree, before the incremental sync overwrites the patched files
/// with pristine worktree contents — `.pc` would still record the patches
/// as applied, making the next source build abort on "unexpected upstream
/// changes" (or a binary build silently compile unpatched sources).
/// `dpkg-source --after-build --unapply-patches` restores the pre-patch
/// state from the `.pc` backups and removes the quilt db entirely.
fn unapply_quilt_patches(
    build: &Build,
    kind: BuildKind,
    package: &SourcePackage,
) -> anyhow::Result<()> {
    let staged_source_dir = stage_dir(&build.environment, kind);
    if !needs_quilt_unapply(package.source_format(), &staged_source_dir) {
        return Ok(());
    }
    build
        .driver
        .run_command_checked(
            &["dpkg-source", "--after-build", "--unapply-patches", "."],
            &staged_source_dir,
            false,
            &[],
        )
        .context("failed to unapply quilt patches left in the build tree")
}

/// Remove every previous leftover from a stage dir (binary residue like
/// `debian/<pkg>/` install trees makes `dpkg-source -b` abort on "unwanted
/// binary file"). Only this dir is wiped — the other kind's tree sharing
/// the environment keeps its incremental outputs.
fn reset_stage_dir(
    build: &Build,
    environment: &Environment,
    kind: BuildKind,
) -> anyhow::Result<()> {
    let dir = stage_dir(environment, kind);
    if !dir.exists() {
        return Ok(());
    }
    build
        .driver
        .run_command_checked(&["find", ".", "-mindepth", "1", "-delete"], &dir, true, &[])
        .context("failed to reset the source stage directory")
}

pub fn get_shell_in_build(config: &Config, package: &SourcePackage) -> anyhow::Result<()> {
    let (_package_identifier, build_root) =
        get_build_root_and_identifier(&config.temp_build_dir, package);
    let build = Build::from_build_root(&build_root, &config.driver)?;
    // the binary tree is the iteration workflow; fall back to the source
    // tree when only source builds ever ran
    let kind = if stage_dir(&build.environment, BuildKind::Binary).exists() {
        BuildKind::Binary
    } else {
        BuildKind::Source
    };
    let result = build
        .driver
        .interactive_shell(&stage_dir(&build.environment, kind));

    build.detach()?;

    result?;
    Ok(())
}

fn deb_build_options(existing: Option<&str>, build_debug_symbols: bool, run_test: bool) -> String {
    let mut options = existing
        .unwrap_or_default()
        .split_whitespace()
        .filter(|option| *option != "noautodbgsym" && *option != "nocheck")
        .collect::<Vec<_>>();
    if !build_debug_symbols {
        options.push("noautodbgsym");
    }
    if !run_test {
        options.push("nocheck");
    }
    options.join(" ")
}

/// Everything needed to run one package build.
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
) -> anyhow::Result<PathBuf> {
    let sign = &request.intent.config.sign;

    let package = &request.target.package;
    crate::output::stage(&format!(
        "Preparing build environment for {} {}",
        package.name(),
        package.version()
    ));
    let build = prepare_build_env(request.intent, request.target)
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

    let result = build_commands(&build).and_then(|()| {
        crate::output::stage("Exporting artifacts");
        let changes_file = artifacts::export_build_artifacts(
            &build.environment.work_dir(),
            &build.output_dir,
            request.intent.kind,
        )?;
        if sign.source.is_enabled() {
            crate::output::stage(&format!("Signing {}", build.environment.package_identifier));
            crate::sign::sign_file(
                &changes_file,
                sign,
                sign.source,
                sign.notify,
                &build.environment.package_identifier,
            )?;
        }
        Ok(changes_file)
    });

    if let Err(error) = &result {
        if request.intent.shell_on_failure && stdout().is_terminal() {
            eprintln!("Build failed: {error}. Dropping into shell...");
            if let Err(shell_error) = build
                .driver
                .interactive_shell(&stage_dir(&build.environment, request.intent.kind))
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
        return result;
    }

    stop_socket_server();
    build
        .driver
        .cleanup()
        .context("failed to clean up build environment")?;
    result
}

pub fn build_package(
    intent: &BuildIntent,
    target: &PackageTarget,
    changes_options: &[String],
) -> anyhow::Result<PathBuf> {
    let request = BuildRequest { intent, target };
    run_build(&request, |build| {
        crate::output::stage("Building binary packages");
        let staged_source_dir = stage_dir(&build.environment, BuildKind::Binary);
        // build-essential is an implicit dependency that `apt-get build-dep`
        // won't resolve, so install it explicitly. No-op when the environment
        // already has it (idempotent, and the bare driver runs on the host).
        build.driver.run_command_checked(
            &["apt-get", "-y", "install", "build-essential"],
            &staged_source_dir,
            true,
            &[],
        )?;
        build.driver.run_command_checked(
            &["apt-get", "-y", "build-dep", "."],
            &staged_source_dir,
            true,
            &[],
        )?;
        let inherited_options = std::env::var("DEB_BUILD_OPTIONS").ok();
        let options = deb_build_options(
            inherited_options.as_deref(),
            build.build_debug_symbols,
            build.run_test,
        );
        let mut env_add = vec![("DEB_BUILD_OPTIONS", options.as_str())];
        if let Some(variant) = build.host_arch_variant.as_deref() {
            env_add.push(("DEB_HOST_ARCH_VARIANT", variant));
        }
        let mut dpkg_buildpackage_args: Vec<String> = vec![
            "dpkg-buildpackage".into(),
            "-us".into(),
            "-uc".into(),
            "-ui".into(),
        ];
        if !build.clean {
            // Non-incremental builds already stage a clean source tree, while
            // incremental builds preserve their outputs intentionally.
            dpkg_buildpackage_args.push("-nc".into());
        }
        for option in changes_options {
            dpkg_buildpackage_args.push(format!("--changes-option={option}"));
        }
        dpkg_buildpackage_args.push("-b".into());
        let dpkg_buildpackage_args: Vec<&str> =
            dpkg_buildpackage_args.iter().map(String::as_str).collect();
        build.driver.run_command_checked(
            &dpkg_buildpackage_args,
            &staged_source_dir,
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

/// Make `source` available at `destination` without copying bytes when
/// avoidable: hardlink (same filesystem), then symlink, then copy as the
/// last resort. dpkg-source only reads the file, so a link is fine.
fn stage_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
    if destination.exists() {
        std::fs::remove_file(destination)
            .with_context(|| format!("failed to remove {}", destination.display()))?;
    }
    if std::fs::hard_link(source, destination).is_ok() {
        return Ok(());
    }
    if std::os::unix::fs::symlink(source, destination).is_ok() {
        return Ok(());
    }
    fs::copy(source, destination).map(|_| ()).with_context(|| {
        format!(
            "failed to stage {} into the build environment",
            source.display()
        )
    })
}

/// Build a `.dsc` + tarball + `.buildinfo` + `.changes` source package.
///
/// If `config.clean` is set, build-dependencies are installed before
/// `dpkg-buildpackage` runs `debian/rules clean` once.
/// `changes_options` are extra `--changes-option=...` arguments passed
/// to `dpkg-buildpackage` verbatim, e.g.
/// `--changes-option=-DVcs-Git=https://...` for git-ubuntu's
/// upload/git correlation.
pub async fn build_source_package(
    intent: &BuildIntent,
    target: &PackageTarget,
    changes_options: &[String],
    include_orig: bool,
) -> anyhow::Result<PathBuf> {
    if intent.driver == DriverType::Bare {
        check_dpkg_buildpackage_available()?;
    }

    // dpkg-source looks for the orig tarball in the parent of the source
    // dir it builds, which inside the environment is the work dir.
    let orig_fetch_dir = intent
        .config
        .temp_build_dir
        .join("orig")
        .join(target.package.name());
    let orig_tarball = crate::upstream::orig::fetch_orig_tarball(
        &intent.config.orig_tarball,
        &target.package,
        &orig_fetch_dir,
    )
    .await
    .with_context(|| {
        format!(
            "fetching the orig tarball for {} {} failed",
            target.package.name(),
            target.package.version().upstream_version()
        )
    })?;

    let request = BuildRequest { intent, target };
    run_build(&request, |build| {
        crate::output::stage("Building source package");
        let staged_source_dir = stage_dir(&build.environment, BuildKind::Source);
        if let Some(tarball) = &orig_tarball {
            // the work dir is the staged source dir's parent, which is where
            // dpkg-source looks for the tarball
            let destination = build
                .environment
                .work_dir()
                .join(tarball.file_name().expect("orig tarball has a file name"));
            stage_file(tarball, &destination)?;
        }
        if build.clean {
            build.driver.run_command_checked(
                &["apt-get", "-y", "build-dep", "."],
                &staged_source_dir,
                true,
                &[],
            )?;
        }
        let mut args: Vec<String> = vec![
            "dpkg-buildpackage".into(),
            "-S".into(),
            "-d".into(),
            "-us".into(),
            "-uc".into(),
            "-ui".into(),
        ];
        if !build.clean {
            args.push("-nc".into());
        }
        // -sa/-sd decide whether the .changes references the orig tarball
        args.push(if include_orig {
            "-sa".into()
        } else {
            "-sd".into()
        });
        for option in changes_options {
            args.push(format!("--changes-option={option}"));
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let result = build
            .driver
            .run_command_checked(&args, &staged_source_dir, false, &[]);
        if result.is_err()
            && needs_quilt_unapply(target.package.source_format(), &staged_source_dir)
        {
            eprintln!(
                "debmagic: hint: debian/patches are still applied in the build tree; the next incremental build unapplies them before syncing sources"
            );
        }
        result?;
        Ok(())
    })
    .context("failed to build source package")
}

/// Inputs for the `debmagic build` command: the resolved
/// intent plus the flags that don't belong in the config.
#[derive(Debug, Clone)]
pub struct BuildCommand {
    pub intent: BuildIntentInput,
    /// `--distro` override; the changelog's single distro when unset.
    pub distro: Option<String>,
    /// `--include-orig` (source builds only).
    pub include_orig: Option<crate::upload::orig::IncludeOrig>,
    /// `--bare-ignore-release`: with the bare driver, build even though
    /// the target distro differs from the host's os-release.
    pub bare_ignore_release: bool,
    /// Extra `--changes-option=...` values passed to dpkg-buildpackage.
    pub changes_options: Vec<String>,
    /// `--upload <target>`: after building (and signing, if enabled),
    /// upload the resulting `.changes` to this target.
    pub upload: Option<String>,
}

/// Run the `debmagic build` command: resolve the intent and target,
/// build the binary or source package, and upload the result when
/// `--upload` was passed.
pub async fn build(command: BuildCommand) -> anyhow::Result<()> {
    let intent = resolve_build_intent(command.intent)?;
    let target = crate::package::resolve_package_target(
        &intent.source_dir,
        command.distro.as_deref(),
        crate::package::distro_resolve_mode_for_driver(
            intent.driver,
            &intent.config.driver.docker.base_images,
            &intent.config.driver.lxd.base_images,
        ),
    )
    .context("failed to determine package target")?;

    let upstream_version = target.package.version().upstream_version().to_string();
    let changes_file = if intent.kind.is_source() {
        let include_orig = match command.include_orig {
            Some(mode) => crate::upload::orig::decide_orig_upload(mode, &intent.source_dir)?,
            None => true,
        };
        build_source_package(&intent, &target, &command.changes_options, include_orig)
            .await
            .context("Building the source package failed")?
    } else {
        let mut target = target;
        if intent.driver == DriverType::Bare && !command.bare_ignore_release {
            target.distro = crate::package::validate_bare_host_target(
                &target.distro,
                Path::new("/etc/os-release"),
            )
            .context("host's /etc/os-release does not match the build target distro")?;
        }
        build_package(&intent, &target, &command.changes_options)
            .context("Building the package failed")?
    };

    if let Some(spec) = &command.upload {
        let upload_target = crate::upload::resolve_target(spec, Some(&intent.config.upload))?;
        crate::output::stage(&format!("Uploading to {}", upload_target.name));
        crate::upload::upload_changes(
            &upload_target,
            spec,
            &changes_file,
            false,
            false,
            Some(upstream_version.as_str()),
        )
        .with_context(|| {
            format!(
                "uploading {} to target '{spec}' failed",
                changes_file.display()
            )
        })?;
    }
    Ok(())
}

/// Run the `debmagic shell` command: attach to the currently active
/// build environment and open an interactive shell in it.
pub fn shell(
    fallback_dir: &Path,
    source_dir: Option<&Path>,
    config_file: Option<&Path>,
) -> anyhow::Result<()> {
    let source_dir = crate::package::resolve_source_dir(fallback_dir, source_dir)?;
    let config = Config::load(Some(&source_dir), config_file)?;
    let identity = crate::package::load_package(&source_dir)?;
    get_shell_in_build(&config, &identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quilt_unapply_needed_only_for_quilt_with_applied_patches() {
        let dir = std::env::temp_dir().join(format!("debmagic-quilt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!needs_quilt_unapply(SourceFormat::Quilt, &dir));
        std::fs::create_dir_all(dir.join(".pc")).unwrap();
        assert!(needs_quilt_unapply(SourceFormat::Quilt, &dir));
        assert!(!needs_quilt_unapply(SourceFormat::Native, &dir));
        assert!(!needs_quilt_unapply(SourceFormat::V1, &dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stage_file_prefers_hardlink() {
        let dir = std::env::temp_dir().join(format!("debmagic-stage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("src.tar");
        std::fs::write(&source, "data").unwrap();
        let destination = dir.join("dest.tar");
        stage_file(&source, &destination).unwrap();
        // same filesystem: a hardlink shares the inode
        assert_eq!(
            std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(&source).unwrap()),
            std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(&destination).unwrap())
        );
        // restaging replaces the destination
        stage_file(&source, &destination).unwrap();
        assert_eq!(
            std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(&source).unwrap()),
            std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(&destination).unwrap())
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn debug_symbol_option_preserves_other_build_options() {
        assert_eq!(
            deb_build_options(Some("nocheck parallel=8"), false, true),
            "parallel=8 noautodbgsym"
        );
        assert_eq!(
            deb_build_options(Some("nocheck noautodbgsym parallel=8"), true, true),
            "parallel=8"
        );
    }

    #[test]
    fn run_test_option_adds_nocheck() {
        assert_eq!(
            deb_build_options(None, false, false),
            "noautodbgsym nocheck"
        );
        assert_eq!(
            deb_build_options(Some("parallel=8"), true, false),
            "parallel=8 nocheck"
        );
        assert_eq!(
            deb_build_options(Some("nocheck parallel=8"), true, true),
            "parallel=8"
        );
    }
}
