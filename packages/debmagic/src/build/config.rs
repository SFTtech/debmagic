use serde::Deserialize;

use crate::build::driver_bare::DriverBareConfig;
use crate::build::driver_docker::DriverDockerConfig;

#[derive(Deserialize, Debug, Default)]
pub struct DriverConfig {
    pub docker: DriverDockerConfig,
    pub bare: DriverBareConfig,
}
