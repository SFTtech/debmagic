use std::{
    collections::HashMap,
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, Deserialize)]
pub enum BuildDriverType {
    Docker,
    Bare,
    // Lxd
}

#[derive(Debug, Clone)]
pub struct PackageDescription {
    pub name: String,
    pub version: String,
    pub source_dir: PathBuf,
}

pub type DriverSpecificBuildMetadata = HashMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildMetadata {
    pub driver: BuildDriverType,
    pub config: BuildConfig,
    pub driver_metadata: DriverSpecificBuildMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub package_identifier: String,
    pub build_root_dir: PathBuf,
    pub source_dir: PathBuf,
    pub output_dir: PathBuf,
    pub dry_run: bool,
    pub distro_version: String,
    pub distro: String,
    pub sign_package: bool,
}

impl BuildConfig {
    pub fn build_identifier(&self) -> String {
        format!(
            "{}-{}-{}",
            self.package_identifier, self.distro, self.distro_version
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

pub trait BuildDriver {
    fn get_build_metadata(&self) -> DriverSpecificBuildMetadata;

    fn run_command(&self, cmd: &[&str], cwd: &Path, requires_root: bool) -> std::io::Result<()>;

    fn cleanup(&self);

    fn drop_into_shell(&self) -> std::io::Result<()>;

    fn driver_type(&self) -> BuildDriverType;
}
