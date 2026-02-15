use core::time;
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::{
    fs,
    io::{self, BufReader, IsTerminal, Read, Write, stdout},
    path::{Path, PathBuf},
    thread,
};

use crate::build::config::DriverOverrides;
use crate::{
    build::{
        common::{BuildConfig, BuildDriver, BuildDriverType, BuildMetadata},
        config::DriverConfig,
        driver_bare::DriverBare,
        driver_docker::DriverDocker,
    },
    config::Config,
    package::PackageDescription,
};
use anyhow::{Context, anyhow};
use debmagic_common::distro::DistroVersion;
use glob::glob;

pub mod common;
pub mod config;
pub mod driver_bare;
pub mod driver_docker;

struct Build {
    config: BuildConfig,
    pub driver: Box<dyn BuildDriver>,
}

fn get_build_driver(
    config: &BuildConfig,
    driver_config: &DriverConfig,
    driver_overrides: &DriverOverrides,
) -> anyhow::Result<Box<dyn BuildDriver>> {
    match config.driver {
        BuildDriverType::Docker => Ok(Box::new(DriverDocker::create(
            config,
            driver_config,
            &driver_overrides.docker,
        )?)),
        BuildDriverType::Bare => Ok(Box::new(DriverBare::create(
            config,
            driver_config,
            &driver_overrides.bare,
        ))),
        // BuildDriverType::Lxd => ...
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
        ))),
        BuildDriverType::Bare => Ok(Box::new(DriverBare::from_build_metadata(
            &metadata.config,
            config,
            metadata,
        ))),
        // BuildDriverType::Lxd => ...
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

        // Try to signal the main debmagic build process that a shell attached
        send_socket_command(build_root, "attach")
            .context("No debmagic build is currently running for this source directory")?;

        Ok(Self {
            config: metadata.config.clone(),
            driver,
        })
    }

    pub fn detach(&self) -> anyhow::Result<()> {
        let build_root = &self.config.build_root_dir;
        send_socket_command(build_root, "detach")?;
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
                            num_attached = std::cmp::max(0, num_attached - 1);
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
    fs::create_dir_all(&dst)?;

    let walker = ignore::WalkBuilder::new(&src)
        .standard_filters(true)
        .hidden(false)
        .filter_entry(|entry| !(entry.path().is_dir() && entry.path().ends_with(".git")))
        .build();

    for entry in walker {
        let entry = entry?;
        let file_type = entry.file_type().ok_or(anyhow!(
            "failed to get file type of {}",
            entry.path().display()
        ))?;

        // get path of entry relative to src
        let relative_path = entry
            .path()
            .strip_prefix(src.as_ref())
            .context("failed to get relative path")?;

        if file_type.is_dir() {
            fs::create_dir_all(dst.as_ref().join(relative_path))?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), dst.as_ref().join(relative_path))
                .context(format!("failed to copy file: {}", entry.path().display()))?;
        }
        // handle hardlinks, symlinks and similar weird filetypes
    }
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
/// If multiple distro versions are specified, an explicit --distro-version is required.
/// If --distro-version is provided, it's validated against the changelog versions.
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
            "changelog contains multiple distributions ({}), please specify which one to build for with --distro-version",
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
    if build_root.exists() {
        fs::remove_dir_all(&build_root)?;
    }

    let distro_version = resolve_distro_version(&package.distro_versions, explicit_distro_version)
        .context("failed to determine distro version")?;

    let build_config = BuildConfig {
        driver: driver_type,
        package_identifier,
        source_dir: package.source_dir.clone(),
        output_dir: output_dir.to_path_buf(),
        build_root_dir: build_root,
        distro: distro_version.clone(),
        sign_package: false,
    };

    build_config
        .create_dirs()
        .context("failed to create build directories")?;

    copy_dir_all(&build_config.source_dir, build_config.build_source_dir())
        .context("failed to copy source tree to build directory")?;

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

pub fn build_package(
    config: &Config,
    package: &PackageDescription,
    driver_type: BuildDriverType,
    driver_overrides: &DriverOverrides,
    output_dir: &Path,
    explicit_distro_version: Option<&str>,
) -> anyhow::Result<()> {
    let build = prepare_build_env(
        config,
        driver_overrides,
        package,
        driver_type,
        output_dir,
        explicit_distro_version,
    )
    .context("failed to prepare build environment")?;
    build
        .write_metadata()
        .context("failed to write build metadata")?;

    let should_exit = Arc::new(Mutex::new(false));
    let socket_server_handle =
        start_socket_server(&build.config.build_root_dir, should_exit.clone())?;

    let result = (|| -> anyhow::Result<()> {
        build.driver.run_command(
            &["apt-get", "-y", "build-dep", "."],
            &build.config.build_source_dir(),
            true,
        )?;
        build.driver.run_command(
            &["dpkg-buildpackage", "-us", "-uc", "-ui", "-nc", "-b"],
            &build.config.build_source_dir(),
            false,
        )?;

        if build.config.sign_package {
            // SIGN .changes and .dsc files
            // changes = *.changes / *.dsc
            // driver.run_command(&["debsign", opts, changes], &build_config.build_source_dir(), false)?;
            // driver.run_command(&["debrsign", opts, username, changes],  &build_config.build_source_dir(), false)?;
        }

        let parent_dir = build.config.build_source_dir().join("..");
        copy_glob(&parent_dir, "*.deb", &build.config.output_dir)?;
        copy_glob(&parent_dir, "*.changes", &build.config.output_dir)?;
        copy_glob(&parent_dir, "*.buildinfo", &build.config.output_dir)?;
        copy_glob(&parent_dir, "*.dsc", &build.config.output_dir)?;

        Ok(())
    })();

    if let Err(e) = result {
        if stdout().is_terminal() {
            eprintln!("Build failed: {e}. Dropping into shell...");
            let res = build
                .driver
                .interactive_shell(&build.config.build_source_dir());
            if let Err(shell_error) = res {
                eprintln!("Dropping into shell failed: {shell_error}");
            }
        } else {
            eprintln!("Build failed: {e}");
        }
        build.driver.cleanup();
        *should_exit.lock().unwrap() = true;
        if !socket_server_handle.is_finished() {
            println!("Waiting for all attached shells to exit...");
        }
        socket_server_handle.join().ok();
        return Err(e);
    }

    // Signal the socket server to exit and wait for it to complete
    *should_exit.lock().unwrap() = true;
    if !socket_server_handle.is_finished() {
        println!("Waiting for all attached shells to exit...");
    }
    socket_server_handle.join().ok();

    build.driver.cleanup();
    Ok(())
}

#[cfg(test)]
mod tests {
    use debmagic_common::distro::Distro;

    use super::*;

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
