use std::{path::Path, process::Command};

use serde::{Deserialize, Serialize};

use crate::driver::{Driver, DriverType, Environment, EnvironmentMetadata, config::DriverConfig};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DriverBareConfig {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverBareConfigOverrides {}

pub struct DriverBare {
    environment: Environment,
    _driver_config: DriverConfig,
}

impl DriverBare {
    pub fn create(
        environment: &Environment,
        driver_config: &DriverConfig,
        _overrides: &DriverBareConfigOverrides,
    ) -> Self {
        Self {
            environment: environment.clone(),
            _driver_config: driver_config.clone(),
        }
    }

    pub fn from_metadata(
        environment: &Environment,
        driver_config: &DriverConfig,
        _metadata: &EnvironmentMetadata,
    ) -> Self {
        Self {
            environment: environment.clone(),
            _driver_config: driver_config.clone(),
        }
    }

    pub(crate) fn sign_changes(
        &self,
        changes_file: &Path,
        _gpg: Option<&crate::signing::GpgForwarding>,
        sign_key: Option<&str>,
    ) -> anyhow::Result<()> {
        crate::signing::check_host_debsign_available()?;
        crate::signing::sign_on_host(changes_file, sign_key)
    }
}

impl Driver for DriverBare {
    fn driver_metadata(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::from([])
    }

    fn run_command(
        &self,
        cmd: &[&str],
        cwd: &Path,
        requires_root: bool,
        env_add: &[(&str, &str)],
    ) -> std::io::Result<i32> {
        let mut full_cmd: Vec<String> = Vec::new();

        let is_root = unsafe { libc::geteuid() == 0 };
        if requires_root && !is_root {
            full_cmd.push("sudo".to_string());
        }

        full_cmd.extend(cmd.iter().map(|s| s.to_string()));

        let mut command = Command::new(&full_cmd[0]);
        command.args(&full_cmd[1..]);

        command.current_dir(cwd);
        command.envs(env_add.iter().copied());

        let status = command.status()?;
        Ok(status.code().unwrap_or(-1))
    }

    fn cleanup(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn interactive_shell(&self, _cwd: &Path) -> std::io::Result<()> {
        println!(
            "source directory of current package build in {}",
            self.environment.staged_source_dir().display()
        );
        Ok(())
    }

    fn driver_type(&self) -> DriverType {
        DriverType::Bare
    }

    fn reset_root(&self) -> std::io::Result<()> {
        if self.environment.root_dir.exists() {
            std::fs::remove_dir_all(&self.environment.root_dir)?;
        }
        Ok(())
    }
}
