use std::{
    collections::HashMap,
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Context;
use clap::ValueEnum;
use debmagic_common::distro::DistroVersion;
use serde::{Deserialize, Serialize};

/// Path at which the build root is bind-mounted inside container-based
/// drivers (Docker, LXD, Incus).
pub const BUILD_DIR_IN_CONTAINER: &str = "/debmagic";

/// Rewrite a path inside the host's build root to the equivalent path inside
/// a container that has it bind-mounted at [`BUILD_DIR_IN_CONTAINER`].
pub fn translate_path_in_container(
    build_root_dir: &Path,
    path_in_source: &Path,
) -> std::io::Result<PathBuf> {
    path_in_source
        .strip_prefix(build_root_dir)
        .map(|rel| Path::new(BUILD_DIR_IN_CONTAINER).join(rel))
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Path is not relative to build root".to_string(),
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
/// name for later reattachment via `from_build_metadata`.
const CONTAINER_NAME_KEY: &str = "container_name";

pub fn container_name_metadata(name: &str) -> DriverSpecificBuildMetadata {
    DriverSpecificBuildMetadata::from([(CONTAINER_NAME_KEY.to_string(), name.to_string())])
}

pub fn container_name_from_metadata(build_metadata: &BuildMetadata) -> anyhow::Result<String> {
    build_metadata
        .driver_metadata
        .get(CONTAINER_NAME_KEY)
        .cloned()
        .context("build metadata has no container_name")
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, Deserialize)]
pub enum BuildDriverType {
    Docker,
    Bare,
    Lxd,
    Incus,
}

pub type DriverSpecificBuildMetadata = HashMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildMetadata {
    pub config: BuildConfig,
    pub driver_metadata: DriverSpecificBuildMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub driver: BuildDriverType,

    #[serde(default)]
    pub package_name: String,
    pub package_identifier: String,
    pub build_root_dir: PathBuf,
    pub source_dir: PathBuf,
    pub output_dir: PathBuf,
    pub distro: DistroVersion,
    pub sign_package: bool,
    /// GPG key ID/email to sign with (debsign's `-k` option).
    pub sign_key: Option<String>,
    /// Build the automatic `-dbgsym` debug symbol package alongside the regular binaries.
    #[serde(default)]
    pub build_debug_symbols: bool,
    /// Run `debian/rules clean` before building.
    #[serde(default)]
    pub clean: bool,
    /// Keep the build environment running after the build.
    #[serde(default)]
    pub persistent: bool,
    /// Synchronize source inputs while preserving build-generated files.
    #[serde(default)]
    pub incremental: bool,
}

impl BuildConfig {
    pub fn build_identifier(&self) -> String {
        format!(
            "{}-{}-{}",
            self.package_identifier, self.distro.distro, self.distro.codename
        )
    }

    pub fn build_work_dir(&self) -> PathBuf {
        self.build_root_dir.join("work")
    }

    pub fn build_temp_dir(&self) -> PathBuf {
        self.build_root_dir.join("temp")
    }

    pub fn build_source_dir(&self) -> PathBuf {
        self.build_work_dir().join(&self.package_identifier)
    }

    pub fn create_dirs(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.output_dir)?;
        fs::create_dir_all(self.build_work_dir())?;
        fs::create_dir_all(self.build_temp_dir())?;
        fs::create_dir_all(self.build_source_dir())?;
        Ok(())
    }
}

/// Source of a Python script that rewrites the default Debian/Ubuntu apt
/// sources to point at a mirror. Replaces the base image's default sources
/// file(s) outright with one debmagic owns, rather than parsing and
/// patching them in place, defaulting to the release/updates/security
/// pockets it detects from `/etc/os-release`.
pub const APT_MIRROR_SCRIPT: &str = include_str!("scripts/mirror.py");

pub trait BuildDriver {
    fn get_build_metadata(&self) -> DriverSpecificBuildMetadata;

    fn run_command_env(
        &self,
        cmd: &[&str],
        cwd: &Path,
        requires_root: bool,
        env_add: &[(&str, &str)],
    ) -> std::io::Result<()>;

    fn run_command(&self, cmd: &[&str], cwd: &Path, requires_root: bool) -> std::io::Result<()> {
        self.run_command_env(cmd, cwd, requires_root, &[])
    }

    fn cleanup(&self) -> anyhow::Result<()>;

    fn interactive_shell(&self, cwd: &Path) -> std::io::Result<()>;

    fn driver_type(&self) -> BuildDriverType;

    fn reset_build_root(&self) -> std::io::Result<()>;

    fn reused_environment(&self) -> bool {
        true
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

    fn sample_config(package_identifier: &str) -> BuildConfig {
        BuildConfig {
            driver: BuildDriverType::Docker,
            package_identifier: package_identifier.to_string(),
            build_root_dir: PathBuf::from("/tmp"),
            source_dir: PathBuf::from("/tmp/src"),
            output_dir: PathBuf::from("/tmp/out"),
            distro: DistroVersion {
                distro: Distro::Debian,
                codename: "forky".to_string(),
                version: "15".to_string(),
                is_devel: false,
            },
            sign_package: false,
            incremental: false,
            build_debug_symbols: false,
            clean: false,
            persistent: false,
            package_name: "debmagic".to_string(),
            sign_key: None,
        }
    }

    #[test]
    fn docker_identifier_replaces_debian_prerelease_tilde() {
        let config = sample_config("debmagic-0.0.1~alpha2");
        assert_eq!(
            config.build_identifier(),
            "debmagic-0.0.1~alpha2-debian-forky"
        );
    }
}
