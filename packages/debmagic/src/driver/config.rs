use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::driver::DriverType;
use crate::driver::driver_bare::{DriverBareConfig, DriverBareConfigOverrides};
use crate::driver::driver_docker::{DriverDockerConfig, DriverDockerConfigOverrides};
use crate::driver::driver_lxd::{DriverLxdConfig, DriverLxdConfigOverrides};

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

    pub fn from_name(name: &str) -> anyhow::Result<Self> {
        match name {
            "no" => Ok(Self::No),
            "on-failure" => Ok(Self::OnFailure),
            "always" => Ok(Self::Always),
            other => anyhow::bail!("unknown Persistence '{other}'"),
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
    pub docker: DriverDockerConfig,
    pub bare: DriverBareConfig,
    pub lxd: DriverLxdConfig,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct DriverOverrides {
    pub docker: DriverDockerConfigOverrides,
    pub bare: DriverBareConfigOverrides,
    pub lxd: DriverLxdConfigOverrides,
}
