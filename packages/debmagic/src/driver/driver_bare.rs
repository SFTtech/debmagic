use std::{path::Path, process::Command};

use serde::{Deserialize, Serialize};

use crate::driver::{
    DriverType, Environment, EnvironmentDriver, EnvironmentMetadata, IsolationCapability,
    ResourceStatus, config::DriverConfig,
};
use crate::subprocess::{self, Capture, CommandResult};

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
}

impl EnvironmentDriver for DriverBare {
    fn driver_metadata(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::from([])
    }

    fn run_command(
        &self,
        cmd: &[&str],
        cwd: &Path,
        requires_root: bool,
        env_add: &[(&str, &str)],
        capture: Capture,
    ) -> std::io::Result<CommandResult> {
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

        subprocess::command(command).capture(capture).run()
    }

    fn interactive_shell(&self, cwd: &Path) -> std::io::Result<()> {
        let shell =
            std::env::var_os("SHELL").unwrap_or_else(|| std::ffi::OsString::from("/bin/sh"));
        let mut command = Command::new(shell);
        command.current_dir(cwd);
        let status = command.status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!("shell exited with {status}")));
        }
        Ok(())
    }

    fn driver_type(&self) -> DriverType {
        DriverType::Bare
    }

    fn isolation_capability(&self) -> IsolationCapability {
        IsolationCapability::None
    }

    fn reset_root(&self) -> std::io::Result<()> {
        if self.environment.root_dir.exists() {
            std::fs::remove_dir_all(&self.environment.root_dir)?;
        }
        Ok(())
    }

    fn probe_resource(&self) -> ResourceStatus {
        if self.environment.root_dir.exists() {
            ResourceStatus::Present
        } else {
            ResourceStatus::Absent
        }
    }

    fn destroy_resource(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
