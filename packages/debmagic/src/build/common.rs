use std::{
    collections::HashMap,
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
};

use clap::ValueEnum;
use debmagic_common::distro::DistroVersion;
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, Deserialize)]
pub enum BuildDriverType {
    Docker,
    Bare,
    // Lxd
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

    pub package_identifier: String,
    pub build_root_dir: PathBuf,
    pub source_dir: PathBuf,
    pub output_dir: PathBuf,
    pub distro: DistroVersion,
    pub sign_package: bool,
}

impl BuildConfig {
    pub fn build_identifier(&self) -> String {
        format!(
            "{}-{}-{}",
            self.package_identifier, self.distro.distro, self.distro.codename
        )
    }

    /// Identifier safe for Docker image tags and container names.
    ///
    /// Debian versions may contain `~`, `+`, or `:` (e.g. `0.0.1~alpha2`), which
    /// are invalid in Docker references.
    pub fn docker_identifier(&self) -> String {
        sanitize_docker_reference(&self.build_identifier())
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

/// Replace characters that Docker image/container names disallow.
fn sanitize_docker_reference(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => c,
            _ => '-',
        })
        .collect()
}

pub trait BuildDriver {
    fn get_build_metadata(&self) -> DriverSpecificBuildMetadata;

    fn run_command(&self, cmd: &[&str], cwd: &Path, requires_root: bool) -> std::io::Result<()>;

    fn cleanup(&self);

    fn interactive_shell(&self, cwd: &Path) -> std::io::Result<()>;

    fn driver_type(&self) -> BuildDriverType;
}

#[cfg(test)]
mod tests {
    use super::*;
    use debmagic_common::distro::{Distro, DistroVersion};
    use std::path::PathBuf;

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
            },
            sign_package: false,
        }
    }

    #[test]
    fn docker_identifier_replaces_debian_prerelease_tilde() {
        let config = sample_config("debmagic-0.0.1~alpha2");
        assert_eq!(
            config.build_identifier(),
            "debmagic-0.0.1~alpha2-debian-forky"
        );
        assert_eq!(
            config.docker_identifier(),
            "debmagic-0.0.1-alpha2-debian-forky"
        );
    }

    #[test]
    fn docker_identifier_replaces_epoch_and_plus() {
        let config = sample_config("pkg-1:2.0.0+dfsg1");
        assert_eq!(config.docker_identifier(), "pkg-1-2.0.0-dfsg1-debian-forky");
    }

    #[test]
    fn sanitize_docker_reference_keeps_allowed_chars() {
        assert_eq!(
            sanitize_docker_reference("debmagic-0.0.1-alpha1_x86"),
            "debmagic-0.0.1-alpha1_x86"
        );
    }
}
