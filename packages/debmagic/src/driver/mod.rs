use std::{
    collections::HashMap,
    fmt::Debug,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Context;
use clap::ValueEnum;
use debmagic_common::distro::DistroVersion;
use serde::{Deserialize, Serialize};

use crate::driver::{
    config::{DriverConfig, DriverOverrides},
    driver_bare::DriverBare,
    driver_docker::DriverDocker,
    driver_lxd::{DriverLxd, LxdVariant},
};

pub mod config;
pub mod driver_bare;
pub mod driver_docker;
pub mod driver_lxd;

/// Path at which the environment root is bind-mounted inside container-based
/// drivers (Docker, LXD, Incus).
pub const ENVIRONMENT_DIR_IN_CONTAINER: &str = "/debmagic";

/// Rewrite a path inside the host's environment root to the equivalent path
/// inside a container that has it bind-mounted at [`ENVIRONMENT_DIR_IN_CONTAINER`].
pub fn translate_path_in_container(root_dir: &Path, path_in_source: &Path) -> io::Result<PathBuf> {
    path_in_source
        .strip_prefix(root_dir)
        .map(|rel| Path::new(ENVIRONMENT_DIR_IN_CONTAINER).join(rel))
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Path is not relative to environment root".to_string(),
            )
        })
}

/// Run `cmd`, failing with `context` (and, on a clean but unsuccessful exit,
/// its exit status) if it can't be spawned or exits unsuccessfully.
pub fn run_checked(cmd: &mut Command, context: &str) -> anyhow::Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("Error running {context}"))?;
    if !status.success() {
        anyhow::bail!("{context} failed (exit status: {status})");
    }
    Ok(())
}

pub fn resource_name(prefix: &str, label: &str, identifier: &str) -> String {
    const MAX_LEN: usize = 63;
    const HASH_LEN: usize = 16;

    let mut hash = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, identifier.as_bytes())
        .simple()
        .to_string();
    hash.truncate(HASH_LEN);
    let max_label_len = MAX_LEN - prefix.len() - hash.len() - 2;
    let label = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(max_label_len)
        .collect::<String>();
    format!("{prefix}-{label}-{hash}")
}

pub fn environment_fingerprint(parts: &[&str]) -> String {
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, parts.join("\0").as_bytes())
        .simple()
        .to_string()
}

/// Metadata key under which container-based drivers store their container's
/// name for later reattachment via `create_driver_from_metadata`.
const CONTAINER_NAME_KEY: &str = "container_name";

pub fn container_name_metadata(name: &str) -> HashMap<String, String> {
    HashMap::from([(CONTAINER_NAME_KEY.to_string(), name.to_string())])
}

pub fn container_name_from_metadata(metadata: &EnvironmentMetadata) -> anyhow::Result<String> {
    metadata
        .driver_metadata
        .get(CONTAINER_NAME_KEY)
        .cloned()
        .context("environment metadata has no container_name")
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, Deserialize)]
pub enum DriverType {
    Docker,
    Bare,
    Lxd,
    Incus,
}

/// Isolation an Environment actually provides for a TestRun.
///
/// A ladder: none, then container, then machine. An Environment advertises
/// its rung and every rung below. The Driver that created the Environment
/// reports the rung; it must not claim a rung it does not provide.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum IsolationCapability {
    #[default]
    None,
    Container,
    Machine,
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentPurpose {
    #[default]
    Build,
    Test,
}

impl EnvironmentPurpose {
    /// Extra part for environment fingerprints when purpose is not [`Self::Build`].
    pub fn fingerprint_part(self) -> Option<&'static str> {
        match self {
            Self::Build => None,
            Self::Test => Some("test"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub driver: DriverType,
    #[serde(default)]
    pub package_name: String,
    pub package_identifier: String,
    pub root_dir: PathBuf,
    pub distro: DistroVersion,
    #[serde(default)]
    pub persistent: bool,
    #[serde(default)]
    pub purpose: EnvironmentPurpose,
}

impl Environment {
    pub fn identifier(&self) -> String {
        let base = format!(
            "{}-{}-{}",
            self.package_identifier, self.distro.distro, self.distro.codename
        );
        match self.purpose {
            EnvironmentPurpose::Build => base,
            EnvironmentPurpose::Test => format!("{base}-test"),
        }
    }

    pub fn work_dir(&self) -> PathBuf {
        self.root_dir.join("work")
    }

    pub fn temp_dir(&self) -> PathBuf {
        self.root_dir.join("temp")
    }

    pub fn staged_source_dir(&self) -> PathBuf {
        self.work_dir().join(&self.package_identifier)
    }

    pub fn create_dirs(&self) -> io::Result<()> {
        fs::create_dir_all(self.work_dir())?;
        fs::create_dir_all(self.temp_dir())?;
        fs::create_dir_all(self.staged_source_dir())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentMetadata {
    pub environment: Environment,
    pub driver_metadata: HashMap<String, String>,
}

/// Source of a Python script that rewrites the default Debian/Ubuntu apt
/// sources to point at a mirror. Replaces the base image's default sources
/// file(s) outright with one debmagic owns, rather than parsing and
/// patching them in place, defaulting to the release/updates/security
/// pockets it detects from `/etc/os-release`.
pub const APT_MIRROR_SCRIPT: &str = include_str!("scripts/mirror.py");

pub trait Driver {
    fn driver_metadata(&self) -> HashMap<String, String>;

    fn run_command(
        &self,
        cmd: &[&str],
        cwd: &Path,
        requires_root: bool,
        env_add: &[(&str, &str)],
    ) -> io::Result<i32>;

    fn run_command_checked(
        &self,
        cmd: &[&str],
        cwd: &Path,
        requires_root: bool,
        env_add: &[(&str, &str)],
    ) -> io::Result<()> {
        let code = self.run_command(cmd, cwd, requires_root, env_add)?;
        if code != 0 {
            return Err(io::Error::other(format!(
                "Command failed with exit code: {code}"
            )));
        }
        Ok(())
    }

    fn cleanup(&self) -> anyhow::Result<()>;

    fn interactive_shell(&self, cwd: &Path) -> io::Result<()>;

    fn driver_type(&self) -> DriverType;

    /// Isolation this Driver's Environment actually provides.
    fn isolation_capability(&self) -> IsolationCapability;

    fn reset_root(&self) -> io::Result<()>;

    fn reused_environment(&self) -> bool {
        true
    }
}

pub enum DriverInstance {
    Docker(DriverDocker),
    Bare(DriverBare),
    Lxd(DriverLxd),
}

impl Driver for DriverInstance {
    fn driver_metadata(&self) -> HashMap<String, String> {
        match self {
            Self::Docker(d) => d.driver_metadata(),
            Self::Bare(d) => d.driver_metadata(),
            Self::Lxd(d) => d.driver_metadata(),
        }
    }

    fn run_command(
        &self,
        cmd: &[&str],
        cwd: &Path,
        requires_root: bool,
        env_add: &[(&str, &str)],
    ) -> io::Result<i32> {
        match self {
            Self::Docker(d) => d.run_command(cmd, cwd, requires_root, env_add),
            Self::Bare(d) => d.run_command(cmd, cwd, requires_root, env_add),
            Self::Lxd(d) => d.run_command(cmd, cwd, requires_root, env_add),
        }
    }

    fn cleanup(&self) -> anyhow::Result<()> {
        match self {
            Self::Docker(d) => d.cleanup(),
            Self::Bare(d) => d.cleanup(),
            Self::Lxd(d) => d.cleanup(),
        }
    }

    fn interactive_shell(&self, cwd: &Path) -> io::Result<()> {
        match self {
            Self::Docker(d) => d.interactive_shell(cwd),
            Self::Bare(d) => d.interactive_shell(cwd),
            Self::Lxd(d) => d.interactive_shell(cwd),
        }
    }

    fn driver_type(&self) -> DriverType {
        match self {
            Self::Docker(d) => d.driver_type(),
            Self::Bare(d) => d.driver_type(),
            Self::Lxd(d) => d.driver_type(),
        }
    }

    fn isolation_capability(&self) -> IsolationCapability {
        match self {
            Self::Docker(d) => d.isolation_capability(),
            Self::Bare(d) => d.isolation_capability(),
            Self::Lxd(d) => d.isolation_capability(),
        }
    }

    fn reset_root(&self) -> io::Result<()> {
        match self {
            Self::Docker(d) => d.reset_root(),
            Self::Bare(d) => d.reset_root(),
            Self::Lxd(d) => d.reset_root(),
        }
    }

    fn reused_environment(&self) -> bool {
        match self {
            Self::Docker(d) => d.reused_environment(),
            Self::Bare(d) => d.reused_environment(),
            Self::Lxd(d) => d.reused_environment(),
        }
    }
}

impl DriverInstance {
    pub fn sign_changes(
        &self,
        changes_file: &Path,
        gpg: Option<&crate::signing::GpgForwarding>,
        sign_key: Option<&str>,
    ) -> anyhow::Result<()> {
        match self {
            Self::Docker(d) => d.sign_changes(changes_file, gpg, sign_key),
            Self::Bare(d) => d.sign_changes(changes_file, gpg, sign_key),
            Self::Lxd(d) => d.sign_changes(changes_file, gpg, sign_key),
        }
    }
}

pub fn create_driver(
    environment: &Environment,
    driver_config: &DriverConfig,
    overrides: &DriverOverrides,
) -> anyhow::Result<DriverInstance> {
    let apt_mirror = overrides
        .apt_mirror
        .as_deref()
        .or(driver_config.apt_mirror.as_deref());
    let proposed = overrides.proposed.unwrap_or(driver_config.proposed);

    match environment.driver {
        DriverType::Docker => Ok(DriverInstance::Docker(DriverDocker::create(
            environment,
            driver_config,
            &overrides.docker,
            apt_mirror,
            proposed,
        )?)),
        DriverType::Bare => Ok(DriverInstance::Bare(DriverBare::create(
            environment,
            driver_config,
            &overrides.bare,
        ))),
        DriverType::Lxd | DriverType::Incus => {
            let variant = match environment.driver {
                DriverType::Lxd => LxdVariant::Lxd,
                _ => LxdVariant::Incus,
            };
            Ok(DriverInstance::Lxd(DriverLxd::create(
                variant,
                environment,
                driver_config,
                &overrides.lxd,
                apt_mirror,
                proposed,
            )?))
        }
    }
}

pub fn create_driver_from_metadata(
    driver_config: &DriverConfig,
    metadata: &EnvironmentMetadata,
) -> anyhow::Result<DriverInstance> {
    match metadata.environment.driver {
        DriverType::Docker => Ok(DriverInstance::Docker(DriverDocker::from_metadata(
            &metadata.environment,
            driver_config,
            metadata,
        )?)),
        DriverType::Bare => Ok(DriverInstance::Bare(DriverBare::from_metadata(
            &metadata.environment,
            driver_config,
            metadata,
        ))),
        DriverType::Lxd | DriverType::Incus => {
            let variant = match metadata.environment.driver {
                DriverType::Lxd => LxdVariant::Lxd,
                _ => LxdVariant::Incus,
            };
            Ok(DriverInstance::Lxd(DriverLxd::from_metadata(
                variant,
                &metadata.environment,
                driver_config,
                metadata,
            )?))
        }
    }
}

/// Remove `root` from the host. If files are owned by a container user the host
/// cannot delete, delete them from inside that environment first. Never requires
/// host root.
pub fn remove_environment_root(root: &Path, driver_config: &DriverConfig) -> anyhow::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    match fs::remove_dir_all(root) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            let metadata_path = root.join("environment.json");
            if !metadata_path.is_file() {
                return Err(e).with_context(|| {
                    format!(
                        "failed to remove {} (permission denied) and no environment.json is present to delete files from inside the environment",
                        root.display()
                    )
                });
            }
            let file = fs::OpenOptions::new()
                .read(true)
                .open(&metadata_path)
                .with_context(|| format!("failed to open {}", metadata_path.display()))?;
            let metadata: EnvironmentMetadata =
                serde_json::from_reader(std::io::BufReader::new(&file))
                    .with_context(|| format!("failed to parse {}", metadata_path.display()))?;
            let driver =
                create_driver_from_metadata(driver_config, &metadata).with_context(|| {
                    format!(
                        "failed to reattach to the environment at {} to delete privileged files",
                        root.display()
                    )
                })?;
            driver.reset_root().with_context(|| {
                format!(
                    "failed to delete files inside the environment at {}",
                    root.display()
                )
            })?;
            fs::remove_dir_all(root).with_context(|| {
                format!(
                    "failed to remove {} after deleting its contents from inside the environment",
                    root.display()
                )
            })?;
            Ok(())
        }
        Err(e) => Err(e).with_context(|| format!("failed to remove {}", root.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use debmagic_common::distro::{Distro, DistroVersion};
    use std::path::PathBuf;

    #[test]
    fn resource_names_are_valid_stable_and_distinct() {
        let first = resource_name("debmagic", "package", "package-1.0~beta-1:2-debian-forky");
        let second = resource_name("debmagic", "package", "package-1.0-beta-1:2-debian-forky");

        assert_eq!(
            first,
            resource_name("debmagic", "package", "package-1.0~beta-1:2-debian-forky")
        );
        assert_ne!(first, second);
        assert!(first.starts_with("debmagic-package-"));
        assert_eq!(first.len(), "debmagic-package-".len() + 16);
        assert!(first.len() <= 63);
        assert!(first.chars().all(|character| character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '-'));
    }

    fn sample_environment(package_identifier: &str) -> Environment {
        Environment {
            driver: DriverType::Docker,
            package_identifier: package_identifier.to_string(),
            root_dir: PathBuf::from("/tmp"),
            distro: DistroVersion {
                distro: Distro::Debian,
                codename: "forky".to_string(),
                version: "15".to_string(),
                is_devel: false,
            },
            persistent: false,
            package_name: "debmagic".to_string(),
            purpose: EnvironmentPurpose::Build,
        }
    }

    #[test]
    fn identifier_unchanged_for_build_purpose() {
        let environment = sample_environment("debmagic-0.0.1~alpha2");
        assert_eq!(
            environment.identifier(),
            "debmagic-0.0.1~alpha2-debian-forky"
        );
    }

    #[test]
    fn identifier_differs_for_test_purpose() {
        let mut environment = sample_environment("debmagic-0.0.1~alpha2");
        environment.purpose = EnvironmentPurpose::Test;
        assert_eq!(
            environment.identifier(),
            "debmagic-0.0.1~alpha2-debian-forky-test"
        );
        assert_ne!(
            environment.identifier(),
            sample_environment("debmagic-0.0.1~alpha2").identifier()
        );
    }

    #[test]
    fn environment_without_purpose_deserializes_as_build() {
        let json = r#"{
            "driver": "Docker",
            "package_identifier": "pkg-1.0",
            "root_dir": "/tmp/build",
            "distro": { "distro": "Debian", "codename": "forky", "version": "15" }
        }"#;
        let environment: Environment = serde_json::from_str(json).unwrap();
        assert_eq!(environment.purpose, EnvironmentPurpose::Build);
    }

    #[test]
    fn isolation_capability_is_a_ladder() {
        assert!(IsolationCapability::None < IsolationCapability::Container);
        assert!(IsolationCapability::Container < IsolationCapability::Machine);
        assert!(IsolationCapability::None < IsolationCapability::Machine);
    }
}
