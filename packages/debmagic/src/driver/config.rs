use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::driver::DriverType;
use crate::driver::driver_bare::{DriverBareConfig, DriverBareConfigOverrides};
use crate::driver::driver_docker::{DriverDockerConfig, DriverDockerConfigOverrides};
use crate::driver::driver_lxd::{DriverLxdConfig, DriverLxdConfigOverrides};
use crate::time::RefreshPolicy;

/// How long the current claim lets an Environment outlive that claim.
///
/// The claim replaces whatever the registry recorded. `on-failure` and
/// `always` reuse a kept Environment; `no` discards it.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Persistence {
    /// Torn down when the claim finishes, discarding one already kept.
    No,
    /// Kept only when the build or TestRun command itself fails.
    #[default]
    OnFailure,
    /// Kept when the claim finishes.
    Always,
}

impl Persistence {
    /// Whether this claim reuses an Environment a previous claim kept.
    pub fn reuses(self) -> bool {
        matches!(self, Self::OnFailure | Self::Always)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::OnFailure => "on-failure",
            Self::Always => "always",
        }
    }

    /// Registry encoding. Version 1 stored a boolean: `0` no, `1` always.
    pub fn from_i64(value: i64) -> anyhow::Result<Self> {
        match value {
            0 => Ok(Self::No),
            1 => Ok(Self::Always),
            2 => Ok(Self::OnFailure),
            other => anyhow::bail!("unknown Persistence value {other}"),
        }
    }

    pub fn as_i64(self) -> i64 {
        match self {
            Self::No => 0,
            Self::Always => 1,
            Self::OnFailure => 2,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct DriverConfig {
    /// Default driver when `--driver` is not passed. Binary builds still
    /// require a driver, from the CLI or here; source builds fall back to
    /// `bare`.
    pub default: Option<DriverType>,
    /// Lifetime of the Environment after this claim. Unset is `on-failure`.
    pub persistent: Persistence,
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
