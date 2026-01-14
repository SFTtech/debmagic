use std::{path::Path, process::Command};

use serde::{Deserialize, Serialize};

use crate::build::common::{
    BuildConfig, BuildDriver, BuildDriverType, BuildMetadata, DriverSpecificBuildMetadata,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverBareConfig {}

pub struct DriverBare {
    config: BuildConfig,
    _driver_config: DriverBareConfig,
}

impl DriverBare {
    pub fn create(config: &BuildConfig, driver_config: &DriverBareConfig) -> Self {
        Self {
            config: config.clone(),
            _driver_config: driver_config.clone(),
        }
    }

    pub fn from_build_metadata(
        config: &BuildConfig,
        driver_config: &DriverBareConfig,
        _build_metadata: &BuildMetadata,
    ) -> Self {
        Self {
            config: config.clone(),
            _driver_config: driver_config.clone(),
        }
    }
}

impl BuildDriver for DriverBare {
    fn get_build_metadata(&self) -> DriverSpecificBuildMetadata {
        DriverSpecificBuildMetadata::from([])
    }

    fn run_command(&self, cmd: &[&str], cwd: &Path, requires_root: bool) -> std::io::Result<()> {
        let mut full_cmd: Vec<String> = Vec::new();

        let is_root = unsafe { libc::geteuid() == 0 };
        if requires_root && !is_root {
            full_cmd.push("sudo".to_string());
        }

        full_cmd.extend(cmd.iter().map(|s| s.to_string()));

        if self.config.dry_run {
            println!("[dry-run] Would run: {full_cmd:?}");
            return Ok(());
        }

        let mut command = Command::new(&full_cmd[0]);
        command.args(&full_cmd[1..]);

        command.current_dir(cwd);

        let status = command.status()?;

        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "Command failed with exit code: {:?}",
                status.code()
            )))
        }
    }

    fn cleanup(&self) {
        // No-op for bare driver
    }

    fn interactive_shell(&self) -> std::io::Result<()> {
        println!(
            "source directory of current package build in {}",
            self.config.build_source_dir().display()
        );
        Ok(())
    }

    fn driver_type(&self) -> BuildDriverType {
        BuildDriverType::Bare
    }
}
