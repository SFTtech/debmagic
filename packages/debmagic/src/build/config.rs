use serde::Deserialize;

use crate::build::driver_bare::{DriverBareConfig, DriverBareConfigOverrides};
use crate::build::driver_docker::{DriverDockerConfig, DriverDockerConfigOverrides};

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct DriverConfig {
    pub persistent: bool,
    pub docker: DriverDockerConfig,
    pub bare: DriverBareConfig,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct DriverOverrides {
    pub docker: DriverDockerConfigOverrides,
    pub bare: DriverBareConfigOverrides,
}
