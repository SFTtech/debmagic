use serde::Deserialize;

use crate::build::driver_bare::{DriverBareConfig, DriverBareConfigOverrides};
use crate::build::driver_docker::{DriverDockerConfig, DriverDockerConfigOverrides};
use crate::build::driver_lxd::{DriverLxdConfig, DriverLxdConfigOverrides};

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct DriverConfig {
    pub persistent: bool,
    /// Not used by the bare driver, which builds on the host's own sources.
    pub apt_mirror: Option<String>,
    /// Also enable the `<release>-proposed` pocket. Not used by the bare
    /// driver, which builds on the host's own sources.
    pub proposed: bool,
    pub docker: DriverDockerConfig,
    pub bare: DriverBareConfig,
    pub lxd: DriverLxdConfig,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct DriverOverrides {
    pub apt_mirror: Option<String>,
    pub proposed: Option<bool>,
    pub docker: DriverDockerConfigOverrides,
    pub bare: DriverBareConfigOverrides,
    pub lxd: DriverLxdConfigOverrides,
}
