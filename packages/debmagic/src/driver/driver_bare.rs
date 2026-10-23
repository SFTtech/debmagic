use std::{ffi::OsString, path::Path, process::Command};

use serde::{Deserialize, Serialize};

use crate::driver::{
    CommandOptions, DriverType, Environment, EnvironmentDriver, EnvironmentMetadata,
    IsolationCapability, ResourceStatus, config::DriverConfig,
};
use crate::subprocess::{self, CommandResult};

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
        options: CommandOptions<'_>,
    ) -> std::io::Result<CommandResult> {
        let is_root = unsafe { libc::geteuid() == 0 };
        let mut argv: Vec<OsString> = if let Some(user) = options.user {
            ["sudo", "-u", user, "--"].map(OsString::from).into()
        } else if options.requires_root && !is_root {
            vec!["sudo".into()]
        } else {
            Vec::new()
        };
        argv.extend(cmd.iter().map(OsString::from));
        let mut command = Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .current_dir(cwd)
            .envs(options.env_add.iter().copied());
        subprocess::command(command).capture(options.capture).run()
    }

    fn interactive_shell(&self, cwd: &Path, user: Option<&str>) -> std::io::Result<i32> {
        let mut command = match user {
            Some(user) => {
                // -H prevents the caller's SHELL from replacing the target's.
                let mut command = Command::new("sudo");
                command.args(["-H", "-s", "-u", user]);
                command
            }
            None => {
                Command::new(std::env::var_os("SHELL").unwrap_or_else(|| OsString::from("/bin/sh")))
            }
        };
        let status = command.current_dir(cwd).status()?;
        Ok(subprocess::shell_exit_code(status))
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
