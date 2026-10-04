use std::{
    fs, io,
    path::Path,
    process::{Command, Stdio},
};

use crate::build::source::{source_manifest_path, stage_dir, stage_source_tree};
use crate::build_intent::{BuildIntent, BuildIntentInput, BuildKind, resolve_build_intent};
use crate::driver::{Driver, DriverType, Environment, EnvironmentDriver, EnvironmentPurpose};
use crate::environment::{
    ClaimedEnvironment, CommandCompletion, HostRootPolicy, InvocationKind, Registry, claim_command,
};
use crate::package::PackageTarget;
use anyhow::{Context, anyhow};
use debmagic_common::debian::source::SourceFormat;
use debmagic_common::package::SourcePackage;

pub mod artifacts;
pub mod source;

pub use source::SourceSyncMode;

struct PlannedBuild {
    environment: Environment,
    root_policy: HostRootPolicy,
    incremental: bool,
}

fn plan_build(intent: &BuildIntent, target: &PackageTarget) -> PlannedBuild {
    let package = &target.package;
    let environment = Environment::new(
        intent.driver,
        package.name(),
        &format!("{}-{}", package.name(), package.version()),
        &intent.source_dir,
        target.distro.clone(),
        intent.config.driver.persistent,
        EnvironmentPurpose::Build,
        &intent.config.environments_dir,
    );

    // An incremental sync needs a manifest from a previous build of the same
    // kind; without one the tree is reset so no stale files leak into the
    // build. A source build shares the Environment with the binary tree, so
    // only its own stage dir is restaged.
    let manifest = source_manifest_path(&environment, intent.kind);
    let incremental = intent.config.incremental && manifest.is_file();
    let root_policy = if incremental {
        HostRootPolicy::Keep
    } else if intent.kind.is_source() {
        HostRootPolicy::ResetStage {
            stage_dir: stage_dir(&environment, intent.kind),
        }
    } else {
        HostRootPolicy::Reset
    };
    PlannedBuild {
        environment,
        root_policy,
        incremental,
    }
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
    driver: &Driver,
    environment: &Environment,
    kind: BuildKind,
    package: &SourcePackage,
) -> anyhow::Result<()> {
    let staged_source_dir = stage_dir(environment, kind);
    if !needs_quilt_unapply(package.source_format(), &staged_source_dir) {
        return Ok(());
    }
    driver
        .run_command_checked(
            &["dpkg-source", "--after-build", "--unapply-patches", "."],
            &staged_source_dir,
            false,
            &[],
        )
        .context("failed to unapply quilt patches left in the build tree")
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

/// Shared build orchestration: claim the Environment, run `build_commands`
/// in it, export the artifacts to the output dir, and sign them if requested.
/// Recording, the failure shell, and finish belong to [`claim_command`].
fn run_build(
    request: &BuildRequest,
    kind: InvocationKind,
    build_commands: impl FnOnce(&ClaimedEnvironment) -> anyhow::Result<()>,
) -> anyhow::Result<std::path::PathBuf> {
    let sign = &request.intent.config.sign;
    let registry = Registry::open_default_or_ephemeral()?;

    let package = &request.target.package;
    crate::output::stage(&format!(
        "Preparing build environment for {} {}",
        package.name(),
        package.version()
    ));
    let planned = plan_build(request.intent, request.target);
    let build_kind = request.intent.kind;
    let version = package.version().to_string();

    claim_command(
        &registry,
        planned.environment,
        &request.intent.config.driver,
        &request.intent.driver_overrides,
        planned.root_policy,
        |environment, driver| {
            fs::create_dir_all(&request.intent.output_dir)
                .context("failed to create output directory")?;
            if planned.incremental
                && let Some(driver) = driver
            {
                unapply_quilt_patches(driver, environment, build_kind, package)?;
            }
            crate::output::step("Staging source tree");
            stage_source_tree(
                environment,
                build_kind,
                package,
                request.intent.config.source_sync_mode,
                request.intent.config.incremental,
            )
        },
        kind,
        &version,
        |claimed| {
            let built = build_commands(claimed);
            let shell_worthy = built.is_err();
            let result = built.and_then(|()| {
                let environment = claimed.environment();
                crate::output::stage(&format!(
                    "Exporting artifacts to {}",
                    request.intent.output_dir.display()
                ));
                let changes_file = artifacts::export_build_artifacts(
                    &environment.work_dir(),
                    &request.intent.output_dir,
                    build_kind,
                )?;
                if sign.source.is_enabled() {
                    crate::output::stage(&format!("Signing {}", environment.package_identifier));
                    crate::sign::sign_file(
                        &changes_file,
                        sign,
                        sign.source,
                        sign.notify,
                        &environment.package_identifier,
                    )?;
                }
                Ok(changes_file)
            });
            let success = result.is_ok();
            let changes_path = match (&result, kind) {
                (Ok(path), InvocationKind::BinaryBuild) => Some(path.clone()),
                _ => None,
            };
            let (value, failure_lines) = match result {
                Ok(path) => (Ok(path), Vec::new()),
                Err(error) => {
                    let failure_lines = vec![format!("Build failed: {error}")];
                    (Err(error), failure_lines)
                }
            };
            Ok(CommandCompletion {
                value,
                success,
                shell_worthy,
                finish_error_fails_command: success,
                changes_path,
                failure_lines,
            })
        },
    )
}

pub fn build_package(
    intent: &BuildIntent,
    target: &PackageTarget,
    changes_options: &[String],
) -> anyhow::Result<std::path::PathBuf> {
    let request = BuildRequest { intent, target };
    run_build(&request, InvocationKind::BinaryBuild, |claimed| {
        crate::output::stage("Building binary packages");
        let staged_source_dir = stage_dir(claimed.environment(), BuildKind::Binary);
        // build-essential is an implicit dependency that `apt-get build-dep`
        // won't resolve, so install it explicitly. No-op when the environment
        // already has it (idempotent, and the bare driver runs on the host).
        claimed.driver().run_command_checked(
            &["apt-get", "-y", "install", "build-essential"],
            &staged_source_dir,
            true,
            &[],
        )?;
        claimed.driver().run_command_checked(
            &["apt-get", "-y", "build-dep", "."],
            &staged_source_dir,
            true,
            &[],
        )?;
        let inherited_options = std::env::var("DEB_BUILD_OPTIONS").ok();
        let options = deb_build_options(
            inherited_options.as_deref(),
            intent.config.build_debug_symbols,
            intent.config.run_test,
        );
        let mut env_add = vec![("DEB_BUILD_OPTIONS", options.as_str())];
        if let Some(variant) = intent.config.host_arch_variant.as_deref() {
            env_add.push(("DEB_HOST_ARCH_VARIANT", variant));
        }
        let mut dpkg_buildpackage_args: Vec<String> = vec![
            "dpkg-buildpackage".into(),
            "-us".into(),
            "-uc".into(),
            "-ui".into(),
        ];
        if !intent.config.clean {
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
        claimed.driver().run_command_checked(
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
) -> anyhow::Result<std::path::PathBuf> {
    if intent.driver == DriverType::Bare {
        check_dpkg_buildpackage_available()?;
    }

    // dpkg-source looks for the orig tarball in the parent of the source
    // dir it builds, which inside the environment is the work dir. Fetch
    // into the data directory first; `temp_build_dir` no longer exists.
    let orig_fetch_dir = crate::data_dir::data_dir()
        .context("locating the debmagic data directory")?
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
    run_build(&request, InvocationKind::SourceBuild, |claimed| {
        crate::output::stage("Building source package");
        let staged_source_dir = stage_dir(claimed.environment(), BuildKind::Source);
        if let Some(tarball) = &orig_tarball {
            // the work dir is the staged source dir's parent, which is where
            // dpkg-source looks for the tarball
            let destination = claimed.environment().work_dir().join(
                tarball
                    .file_name()
                    .expect("orig tarball has a file name"),
            );
            stage_file(tarball, &destination)?;
        }
        if intent.config.clean {
            claimed.driver().run_command_checked(
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
        if !intent.config.clean {
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
        let result =
            claimed
                .driver()
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
