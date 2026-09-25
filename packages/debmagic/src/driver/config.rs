use serde::{Deserialize, Serialize};

use crate::driver::DriverType;
use crate::driver::driver_bare::{DriverBareConfig, DriverBareConfigOverrides};
use crate::driver::driver_docker::{DriverDockerConfig, DriverDockerConfigOverrides};
use crate::driver::driver_lxd::{DriverLxdConfig, DriverLxdConfigOverrides};
use crate::time::RefreshPolicy;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct DriverConfig {
    /// Default driver when `--driver` is not passed. Binary builds still
    /// require a driver, from the CLI or here; source builds fall back to
    /// `bare`.
    pub default: Option<DriverType>,
    pub persistent: bool,
    /// Not used by the bare driver, which builds on the host's own sources.
    pub apt_mirror: Option<String>,
    /// Also enable the `<release>-proposed` pocket. Not used by the bare
    /// driver, which builds on the host's own sources.
    pub proposed: bool,
    /// How old the apt index in a persistent environment may get before
    /// `apt-get update` runs again; fresh environments always update once.
    /// Not used by the bare driver, which builds on the host's own sources.
    pub apt_update_age: RefreshPolicy,
    pub docker: DriverDockerConfig,
    pub bare: DriverBareConfig,
    pub lxd: DriverLxdConfig,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct DriverOverrides {
    pub apt_mirror: Option<String>,
    pub proposed: Option<bool>,
    pub apt_update_age: Option<RefreshPolicy>,
    pub docker: DriverDockerConfigOverrides,
    pub bare: DriverBareConfigOverrides,
    pub lxd: DriverLxdConfigOverrides,
}
