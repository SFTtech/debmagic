use core::time;
use std::net::Shutdown;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::{
    cmp::Reverse,
    fs,
    io::{self, BufReader, IsTerminal, Read, Write, stdout},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    thread,
};

use crate::build::config::DriverOverrides;
use crate::{
    build::{
        common::{BuildConfig, BuildDriver, BuildDriverType, BuildMetadata, run_checked},
        config::DriverConfig,
        driver_bare::DriverBare,
        driver_docker::DriverDocker,
        driver_lxd::{DriverLxd, LxdVariant},
    },
    config::Config,
    package::PackageDescription,
};
use anyhow::{Context, anyhow, bail};
use debmagic_common::distro::DistroVersion;
use glob::glob;

pub mod artifacts;
pub mod common;
pub mod config;
pub mod driver_bare;
pub mod driver_docker;
pub mod driver_lxd;

struct Build {
    config: BuildConfig,
    pub driver: Box<dyn BuildDriver>,
    attached: bool,
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
        Ok(Self {
            config: config.clone(),
            driver,
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

fn copy_glob(src_dir: &Path, pattern: &str, dest_dir: &Path) -> anyhow::Result<()> {
    let full_pattern = src_dir.join(pattern).to_string_lossy().into_owned();
    for entry in glob(&full_pattern)? {
        let path = entry?;
        if path.is_file() {
            let filename = path.file_name().ok_or(anyhow!(
                "Could not retrieve filename from {}",
                path.display()
            ))?;
            fs::copy(&path, dest_dir.join(filename))?;
        }
    }
    Ok(())
}

fn socket_path_for_build(build_root: &Path) -> PathBuf {
    build_root.join("build.sock")
}

fn start_socket_server(
    build_root: &Path,
    should_exit: Arc<Mutex<bool>>,
) -> anyhow::Result<thread::JoinHandle<()>> {
    let sock = socket_path_for_build(build_root);
    if sock.exists() {
        // try to remove stale socket file
        let _ = fs::remove_file(&sock);
    }

    let listener = UnixListener::bind(&sock)
        .with_context(|| format!("failed to bind unix socket {}", sock.display()))?;

    // Set non-blocking mode so we can check the exit flag
    listener
        .set_nonblocking(true)
        .context("failed to set socket non-blocking")?;

    let handle = thread::spawn(move || {
        let mut num_attached = 0u64;
        loop {
            // Check if we should exit
            let exit_requested = *should_exit.lock().unwrap();
            if exit_requested && num_attached == 0 {
                break;
            }

            match listener.accept() {
                Ok((mut s, _)) => {
                    let mut buf = String::new();
                    if s.read_to_string(&mut buf).is_err() {
                        let _ = s.shutdown(Shutdown::Both);
                        continue;
                    }
                    let cmd = buf.trim();
                    match cmd {
                        "attach" => {
                            num_attached += 1;
                        }
                        "detach" => {
                            num_attached = num_attached.saturating_sub(1);
                        }
                        _ => {}
                    }
                    let _ = s.shutdown(Shutdown::Both);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No connection available, sleep briefly to avoid busy-waiting
                    thread::sleep(time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
        let _ = fs::remove_file(&sock);
    });

    Ok(handle)
}

fn send_socket_command(build_root: &Path, cmd: &str) -> anyhow::Result<()> {
    let sock = socket_path_for_build(build_root);
    let mut stream = UnixStream::connect(&sock)
        .with_context(|| format!("failed to connect to socket {}", sock.display()))?;
    stream
        .write_all(cmd.as_bytes())
        .context("failed to send socket command")?;
    stream.shutdown(Shutdown::Write).ok();
    Ok(())
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> anyhow::Result<()> {
    let entries = source_tree_entries(src.as_ref())?;
    copy_source_entries(src.as_ref(), dst.as_ref(), &entries)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum SourcePathKind {
    Directory,
    File,
    Symlink,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct SourcePath {
    path: PathBuf,
    kind: SourcePathKind,
}

fn validate_source_path(path: &Path) -> anyhow::Result<()> {
    if path.as_os_str().is_empty()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        bail!("invalid source manifest path: {}", path.display());
    }
    Ok(())
}

fn source_tree_entries(src: &Path) -> anyhow::Result<Vec<SourcePath>> {
    let walker = ignore::WalkBuilder::new(src)
        .standard_filters(true)
        .hidden(false)
        .filter_entry(|entry| !(entry.path().is_dir() && entry.path().ends_with(".git")))
        .build();

    let mut entries = Vec::new();
    for entry in walker {
        let entry = entry?;
        let file_type = entry.file_type().ok_or(anyhow!(
            "failed to get file type of {}",
            entry.path().display()
        ))?;
        let relative_path = entry
            .path()
            .strip_prefix(src)
            .context("failed to get relative path")?;
        if relative_path.as_os_str().is_empty() {
            continue;
        }
        let kind = if file_type.is_dir() {
            SourcePathKind::Directory
        } else if file_type.is_file() {
            SourcePathKind::File
        } else if file_type.is_symlink() {
            SourcePathKind::Symlink
        } else {
            return Err(anyhow!(
                "unsupported file type in source tree: {}",
                entry.path().display()
            ));
        };
        entries.push(SourcePath {
            path: relative_path.to_path_buf(),
            kind,
        });
    }
    entries.sort_by_key(|entry| entry.path.components().count());
    Ok(entries)
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn files_match(source: &Path, destination: &Path) -> std::io::Result<bool> {
    let source_metadata = fs::metadata(source)?;
    let destination_metadata = match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if source_metadata.len() != destination_metadata.len()
        || source_metadata.permissions().mode() != destination_metadata.permissions().mode()
    {
        return Ok(false);
    }

    let mut source = BufReader::new(fs::File::open(source)?);
    let mut destination = BufReader::new(fs::File::open(destination)?);
    let mut source_buffer = [0; 8192];
    let mut destination_buffer = [0; 8192];
    loop {
        let source_len = source.read(&mut source_buffer)?;
        let destination_len = destination.read(&mut destination_buffer)?;
        if source_len != destination_len
            || source_buffer[..source_len] != destination_buffer[..destination_len]
        {
            return Ok(false);
        }
        if source_len == 0 {
            return Ok(true);
        }
    }
}

fn copy_source_entries(src: &Path, dst: &Path, entries: &[SourcePath]) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in entries {
        let source = src.join(&entry.path);
        let destination = dst.join(&entry.path);
        match entry.kind {
            SourcePathKind::Directory => match fs::symlink_metadata(&destination) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    remove_path(&destination)?;
                    fs::create_dir_all(&destination)?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir_all(&destination)?;
                }
                Err(error) => return Err(error.into()),
            },
            SourcePathKind::File => {
                if !files_match(&source, &destination)? {
                    remove_path(&destination)?;
                    fs::copy(&source, &destination)
                        .with_context(|| format!("failed to copy file: {}", source.display()))?;
                }
            }
            SourcePathKind::Symlink => {
                let target = fs::read_link(&source)?;
                if fs::read_link(&destination).ok().as_deref() != Some(target.as_path()) {
                    remove_path(&destination)?;
                    symlink(target, &destination)?;
                }
            }
        }
    }
    Ok(())
}

fn source_manifest_path(build_config: &BuildConfig) -> PathBuf {
    build_config.build_root_dir.join("source-manifest.json")
}

fn write_source_manifest(build_config: &BuildConfig, entries: &[SourcePath]) -> anyhow::Result<()> {
    let manifest_path = source_manifest_path(build_config);
    let temporary_path = manifest_path.with_extension("json.tmp");
    fs::write(&temporary_path, serde_json::to_vec_pretty(entries)?)?;
    fs::rename(temporary_path, manifest_path)?;
    Ok(())
}

fn sync_source_tree(build_config: &BuildConfig) -> anyhow::Result<()> {
    let manifest_path = source_manifest_path(build_config);
    let previous: Vec<SourcePath> = serde_json::from_reader(BufReader::new(
        fs::File::open(&manifest_path)
            .with_context(|| format!("failed to open {}", manifest_path.display()))?,
    ))
    .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    for entry in &previous {
        validate_source_path(&entry.path)?;
    }
    let current = source_tree_entries(&build_config.source_dir)?;

    let current_kinds = current
        .iter()
        .map(|entry| (entry.path.as_path(), entry.kind))
        .collect::<std::collections::HashMap<_, _>>();
    let mut stale = previous
        .iter()
        .filter(|entry| current_kinds.get(entry.path.as_path()) != Some(&entry.kind))
        .collect::<Vec<_>>();
    stale.sort_by_key(|entry| Reverse(entry.path.components().count()));
    for entry in stale {
        let destination = build_config.build_source_dir().join(&entry.path);
        if entry.kind == SourcePathKind::Directory
            && !current_kinds.contains_key(entry.path.as_path())
        {
            match fs::remove_dir(&destination) {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        } else {
            remove_path(&destination)?;
        }
    }

    copy_source_entries(
        &build_config.source_dir,
        &build_config.build_source_dir(),
        &current,
    )?;
    write_source_manifest(build_config, &current)
}

fn stage_source_tree(
    build_config: &BuildConfig,
    package: &PackageDescription,
) -> anyhow::Result<()> {
    if build_config.incremental && source_manifest_path(build_config).is_file() {
        sync_source_tree(build_config).context("failed to synchronize source tree")?;
    } else {
        copy_dir_all(&build_config.source_dir, build_config.build_source_dir())
            .context("failed to copy source tree to build directory")?;
        let entries = source_tree_entries(&build_config.source_dir)?;
        write_source_manifest(build_config, &entries)?;
    }

    let source_parent = build_config
        .source_dir
        .parent()
        .ok_or_else(|| anyhow!("source directory has no parent"))?;
    let prefix = format!("{}_{}", package.name, package.version.upstream_version());
    copy_glob(
        source_parent,
        &format!("{prefix}.orig.tar.*"),
        &build_config.build_work_dir(),
    )?;
    copy_glob(
        source_parent,
        &format!("{prefix}.orig-*.tar.*"),
        &build_config.build_work_dir(),
    )?;
    Ok(())
}

fn get_build_root_and_identifier(
    config: &Config,
    package: &PackageDescription,
) -> (String, PathBuf) {
    let package_identifier = format!("{}-{}", package.name, package.version);
    let build_root = config.temp_build_dir.join(&package_identifier);
    (package_identifier, build_root)
}

/// Determine which distro version to use for the build.
///
/// If only one distro version is specified in the changelog, it's used automatically.
/// If multiple distro versions are specified, an explicit --distro is required.
/// If --distro is provided, it's validated against the changelog versions.
fn resolve_distro_version(
    changelog_distros: &[String],
    explicit_distro: Option<&str>,
) -> anyhow::Result<DistroVersion> {
    let resolved_codename = match (changelog_distros.len(), explicit_distro) {
        (0, _) => Err(anyhow!("changelog contains no distributions")),
        (1, None) => Ok(changelog_distros[0].clone()),
        (1, Some(explicit)) => {
            if explicit == changelog_distros[0] {
                Ok(explicit.to_string())
            } else {
                Err(anyhow!(
                    "explicit distro version '{}' conflicts with distribution specified in changelog '{}'",
                    explicit,
                    changelog_distros[0]
                ))
            }
        }
        (_, None) => Err(anyhow!(
            "changelog contains multiple distributions ({}), please specify which one to build for with --distro",
            changelog_distros.join(", ")
        )),
        (_, Some(explicit)) => {
            if changelog_distros.contains(&explicit.to_string()) {
                Ok(explicit.to_string())
            } else {
                Err(anyhow!(
                    "explicit distro version '{}' not found in changelog distributions: {}",
                    explicit,
                    changelog_distros.join(", ")
                ))
            }
        }
    }?;
    let resolved = debmagic_common::distro::get_distro_version(&resolved_codename)
        .ok_or_else(|| anyhow!("unknown distro codename '{}'", resolved_codename))?;
    Ok(resolved)
}

fn prepare_build_env(
    config: &Config,
    driver_overrides: &DriverOverrides,
    package: &PackageDescription,
    driver_type: BuildDriverType,
    output_dir: &Path,
    explicit_distro_version: Option<&str>,
) -> anyhow::Result<Build> {
    let (package_identifier, build_root) = get_build_root_and_identifier(config, package);

    let distro_version = resolve_distro_version(&package.distro_versions, explicit_distro_version)
        .context("failed to determine distro version")?;

    let build_config = BuildConfig {
        driver: driver_type,
        package_name: package.name.clone(),
        package_identifier,
        source_dir: package.source_dir.clone(),
        output_dir: output_dir.to_path_buf(),
        build_root_dir: build_root.clone(),
        distro: distro_version.clone(),
        sign_package: config.sign_package,
        sign_key: config.sign_key.clone(),
        build_debug_symbols: config.build_debug_symbols,
        clean: config.clean,
        persistent: config.driver.persistent,
        incremental: config.incremental,
    };

    if config.driver.persistent && build_root.exists() {
        // For persistent containers, starting first lets root inside delete
        // container-owned files the host user can't remove.
        let build = Build::create(&build_config, &config.driver, driver_overrides)
            .context(format!("failed to create {:?} build driver", driver_type))?;
        if !config.incremental
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
        stage_source_tree(&build_config, package)?;
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
                && let Ok(driver) = create_driver_from_metadata(&config.driver, &metadata)
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

    stage_source_tree(&build_config, package)?;

    let build = Build::create(&build_config, &config.driver, driver_overrides)?;
    Ok(build)
}

pub fn get_shell_in_build(config: &Config, package: &PackageDescription) -> anyhow::Result<()> {
    let (_package_identifier, build_root) = get_build_root_and_identifier(config, package);
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
pub struct BuildRequest<'a> {
    pub config: &'a Config,
    pub package: &'a PackageDescription,
    pub driver_type: BuildDriverType,
    pub driver_overrides: &'a DriverOverrides,
    pub output_dir: &'a Path,
    pub explicit_distro_version: Option<&'a str>,
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
    let build = prepare_build_env(
        request.config,
        request.driver_overrides,
        request.package,
        request.driver_type,
        request.output_dir,
        request.explicit_distro_version,
    )
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
            sign_changes_file(&changes_file, build.config.sign_key.as_deref())?;
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

pub fn build_package(request: &BuildRequest) -> anyhow::Result<()> {
    run_build(request, true, |build| {
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

/// GPG-sign every `.changes` (and its referenced `.dsc`/`.buildinfo`) in
/// `output_dir` with `debsign`. Always runs on the host regardless of the
/// build driver, since signing needs the user's own gpg keyring, which an
/// ephemeral container doesn't have access to.
fn sign_changes_file(changes_file: &Path, sign_key: Option<&str>) -> anyhow::Result<()> {
    check_command_available(
        "debsign",
        "It's part of devscripts; install it and set up a gpg signing key to use --sign.",
    )?;

    let output_dir = changes_file.parent().ok_or_else(|| {
        anyhow!(
            "could not get output directory of {}",
            changes_file.display()
        )
    })?;
    let filename = changes_file
        .file_name()
        .ok_or_else(|| anyhow!("could not get filename of {}", changes_file.display()))?;

    let mut cmd = Command::new("debsign");
    if let Some(key) = sign_key {
        cmd.arg(format!("-k{key}"));
    }
    cmd.arg(filename).current_dir(output_dir);
    run_checked(&mut cmd, &format!("signing {}", changes_file.display()))?;
    Ok(())
}

/// Build a `.dsc` + tarball + `.buildinfo` + `.changes` source package.
///
/// If `config.clean` is set, build-dependencies are installed before
/// `dpkg-buildpackage` runs `debian/rules clean` once.
pub fn build_source_package(request: &BuildRequest) -> anyhow::Result<()> {
    if request.driver_type == BuildDriverType::Bare {
        check_dpkg_buildpackage_available()?;
    }

    run_build(request, false, |build| {
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
    use std::os::unix::fs::MetadataExt;

    use debmagic_common::distro::Distro;

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

    #[test]
    fn incremental_sync_updates_sources_and_preserves_build_outputs() -> anyhow::Result<()> {
        let test_root = std::env::temp_dir().join(format!(
            "debmagic-incremental-test-{}",
            uuid::Uuid::new_v4()
        ));
        let source_dir = test_root.join("source");
        let build_root_dir = test_root.join("build");
        fs::create_dir_all(source_dir.join("cache"))?;
        fs::write(source_dir.join("changed.txt"), "before")?;
        fs::write(source_dir.join("unchanged.txt"), "unchanged")?;
        fs::write(source_dir.join("removed.txt"), "remove me")?;
        fs::write(source_dir.join("cache/input.c"), "source")?;
        symlink("changed.txt", source_dir.join("link"))?;

        let build_config = BuildConfig {
            driver: BuildDriverType::Bare,
            package_name: "example".to_string(),
            package_identifier: "example-1.0".to_string(),
            build_root_dir: build_root_dir.clone(),
            source_dir: source_dir.clone(),
            output_dir: test_root.join("output"),
            distro: debmagic_common::distro::get_distro_version("trixie").unwrap(),
            sign_package: false,
            sign_key: None,
            build_debug_symbols: false,
            clean: false,
            persistent: true,
            incremental: true,
        };
        build_config.create_dirs()?;
        copy_dir_all(&source_dir, build_config.build_source_dir())?;
        write_source_manifest(&build_config, &source_tree_entries(&source_dir)?)?;
        let unchanged_inode =
            fs::metadata(build_config.build_source_dir().join("unchanged.txt"))?.ino();
        fs::write(
            build_config.build_source_dir().join("cache/output.o"),
            "compiled",
        )?;

        fs::write(source_dir.join("changed.txt"), "after")?;
        fs::remove_file(source_dir.join("removed.txt"))?;
        fs::remove_file(source_dir.join("cache/input.c"))?;
        fs::remove_dir(source_dir.join("cache"))?;
        fs::remove_file(source_dir.join("link"))?;
        symlink("added.txt", source_dir.join("link"))?;
        fs::write(source_dir.join("added.txt"), "new")?;

        sync_source_tree(&build_config)?;

        let staged = build_config.build_source_dir();
        assert_eq!(fs::read_to_string(staged.join("changed.txt"))?, "after");
        assert_eq!(fs::read_to_string(staged.join("added.txt"))?, "new");
        assert_eq!(
            fs::metadata(staged.join("unchanged.txt"))?.ino(),
            unchanged_inode
        );
        assert_eq!(fs::read_link(staged.join("link"))?, Path::new("added.txt"));
        assert!(!staged.join("removed.txt").exists());
        assert!(!staged.join("cache/input.c").exists());
        assert_eq!(
            fs::read_to_string(staged.join("cache/output.o"))?,
            "compiled"
        );

        fs::remove_dir_all(test_root)?;
        Ok(())
    }

    #[test]
    fn source_manifest_paths_must_be_relative_and_normal() {
        assert!(validate_source_path(Path::new("debian/control")).is_ok());
        for path in ["", ".", "../outside", "debian/../outside", "/tmp/outside"] {
            assert!(validate_source_path(Path::new(path)).is_err(), "{path}");
        }
    }

    #[test]
    fn test_resolve_distro_version_single_distro_no_explicit() {
        let distros = vec!["forky".to_string()];
        let result = resolve_distro_version(&distros, None);
        assert!(result.is_ok());
        let distro_version = result.unwrap();
        assert_eq!(distro_version.codename, "forky");
        assert_eq!(distro_version.distro, Distro::Debian);
    }

    #[test]
    fn test_resolve_distro_version_single_distro_matching_explicit() {
        let distros = vec!["forky".to_string()];
        let result = resolve_distro_version(&distros, Some("forky"));
        assert!(result.is_ok());
        let distro_version = result.unwrap();
        assert_eq!(distro_version.codename, "forky");
        assert_eq!(distro_version.distro, Distro::Debian);
    }

    #[test]
    fn test_resolve_distro_version_single_distro_conflicting_explicit() {
        let distros = vec!["forky".to_string()];
        let result = resolve_distro_version(&distros, Some("duke"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("conflicts with distribution specified in changelog")
        );
    }

    #[test]
    fn test_resolve_distro_version_multiple_distros_no_explicit() {
        let distros = vec!["forky".to_string(), "duke".to_string()];
        let result = resolve_distro_version(&distros, None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("multiple distributions")
        );
    }

    #[test]
    fn test_resolve_distro_version_multiple_distros_explicit_valid() {
        let distros = vec!["forky".to_string(), "duke".to_string()];
        let result = resolve_distro_version(&distros, Some("duke"));
        assert!(result.is_ok());
        let distro_version = result.unwrap();
        assert_eq!(distro_version.codename, "duke");
        assert_eq!(distro_version.distro, Distro::Debian);
    }

    #[test]
    fn test_resolve_distro_version_multiple_distros_explicit_invalid() {
        let distros = vec!["forky".to_string(), "duke".to_string()];
        let result = resolve_distro_version(&distros, Some("trixie"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not found in changelog distributions")
        );
    }

    #[test]
    fn test_resolve_distro_version_empty_distros() {
        let distros: Vec<String> = vec![];
        let result = resolve_distro_version(&distros, None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("changelog contains no distributions")
        );
    }
}
