use serde::Deserialize;

use crate::build::driver_bare::DriverBareConfig;
use crate::build::driver_docker::DriverDockerConfig;

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct DriverConfig {
    pub persistent: bool,
    pub docker: DriverDockerConfig,
    pub bare: DriverBareConfig,
}
