use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum Distro {
    Debian,
    Ubuntu,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct DistroVersion {
    pub distro: Distro,
    pub codename: String,
    /// numeric or semver version, e.g. "24.04" for ubuntu or "12" for debian
    pub version: String,
}

impl Distro {
    pub fn as_str(&self) -> &'static str {
        match self {
            Distro::Debian => "debian",
            Distro::Ubuntu => "ubuntu",
        }
    }
}

impl std::fmt::Display for Distro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

static DISTRO_INFO_MAP: LazyLock<HashMap<&'static str, DistroVersion>> = LazyLock::new(|| {
    HashMap::from([
        // debian
        (
            "experimental",
            DistroVersion {
                distro: Distro::Debian,
                codename: "experimental".to_string(),
                version: "".to_string(),
            },
        ),
        (
            "unstable",
            DistroVersion {
                distro: Distro::Debian,
                codename: "unstable".to_string(),
                version: "".to_string(),
            },
        ),
        (
            "sid",
            DistroVersion {
                distro: Distro::Debian,
                codename: "unstable".to_string(),
                version: "".to_string(),
            },
        ),
        (
            "testing",
            DistroVersion {
                distro: Distro::Debian,
                codename: "testing".to_string(),
                version: "".to_string(),
            },
        ),
        (
            "duke",
            DistroVersion {
                distro: Distro::Debian,
                codename: "duke".to_string(),
                version: "15".to_string(),
            },
        ),
        (
            "forky",
            DistroVersion {
                distro: Distro::Debian,
                codename: "forky".to_string(),
                version: "14".to_string(),
            },
        ),
        (
            "trixie",
            DistroVersion {
                distro: Distro::Debian,
                codename: "trixie".to_string(),
                version: "13".to_string(),
            },
        ),
        (
            "stable",
            DistroVersion {
                distro: Distro::Debian,
                codename: "trixie".to_string(),
                version: "13".to_string(),
            },
        ),
        (
            "bookworm",
            DistroVersion {
                distro: Distro::Debian,
                codename: "bookworm".to_string(),
                version: "12".to_string(),
            },
        ),
        (
            "oldstable",
            DistroVersion {
                distro: Distro::Debian,
                codename: "bookworm".to_string(),
                version: "12".to_string(),
            },
        ),
        (
            "bullseye",
            DistroVersion {
                distro: Distro::Debian,
                codename: "bullseye".to_string(),
                version: "11".to_string(),
            },
        ),
        (
            "buster",
            DistroVersion {
                distro: Distro::Debian,
                codename: "buster".to_string(),
                version: "10".to_string(),
            },
        ),
        (
            "stretch",
            DistroVersion {
                distro: Distro::Debian,
                codename: "stretch".to_string(),
                version: "9".to_string(),
            },
        ),
        // ubuntu
        (
            "resolute",
            DistroVersion {
                distro: Distro::Ubuntu,
                codename: "resolute".to_string(),
                version: "26.04".to_string(),
            },
        ),
        (
            "devel",
            DistroVersion {
                distro: Distro::Ubuntu,
                codename: "resolute".to_string(),
                version: "26.04".to_string(),
            },
        ),
        (
            "questing",
            DistroVersion {
                distro: Distro::Ubuntu,
                codename: "questing".to_string(),
                version: "25.10".to_string(),
            },
        ),
        (
            "noble",
            DistroVersion {
                distro: Distro::Ubuntu,
                codename: "noble".to_string(),
                version: "24.04".to_string(),
            },
        ),
        (
            "jammy",
            DistroVersion {
                distro: Distro::Ubuntu,
                codename: "jammy".to_string(),
                version: "22.04".to_string(),
            },
        ),
        (
            "focal",
            DistroVersion {
                distro: Distro::Ubuntu,
                codename: "focal".to_string(),
                version: "20.04".to_string(),
            },
        ),
        (
            "bionic",
            DistroVersion {
                distro: Distro::Ubuntu,
                codename: "bionic".to_string(),
                version: "18.04".to_string(),
            },
        ),
        (
            "xenial",
            DistroVersion {
                distro: Distro::Ubuntu,
                codename: "xenial".to_string(),
                version: "16.04".to_string(),
            },
        ),
        (
            "trusty",
            DistroVersion {
                distro: Distro::Ubuntu,
                codename: "trusty".to_string(),
                version: "14.04".to_string(),
            },
        ),
    ])
});

/// Look up a distribution by codename or suite alias.
///
/// Suite aliases are map keys that resolve to a concrete release [`DistroVersion`]:
/// - Debian: `stable` → current stable release, `oldstable` → current oldstable,
///   `sid` → `unstable`
/// - Ubuntu: `devel` → current development release
///
/// Alias targets are maintained manually when Debian/Ubuntu roll.
pub fn get_distro_version(name: &str) -> Option<DistroVersion> {
    DISTRO_INFO_MAP.get(name).cloned()
}
