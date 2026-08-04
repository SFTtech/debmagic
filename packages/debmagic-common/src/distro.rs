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
    /// true for unreleased development releases (affects image selection)
    #[serde(default)]
    pub is_devel: bool,
}

impl DistroVersion {
    fn new(distro: Distro, codename: &str, version: &str) -> Self {
        Self {
            distro,
            codename: codename.to_string(),
            version: version.to_string(),
            is_devel: false,
        }
    }

    fn devel(mut self) -> Self {
        self.is_devel = true;
        self
    }
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
    use Distro::{Debian, Ubuntu};
    HashMap::from([
        // debian
        (
            "experimental",
            DistroVersion::new(Debian, "experimental", ""),
        ),
        ("unstable", DistroVersion::new(Debian, "unstable", "")),
        ("sid", DistroVersion::new(Debian, "sid", "")),
        ("testing", DistroVersion::new(Debian, "testing", "")),
        ("duke", DistroVersion::new(Debian, "duke", "15")),
        ("forky", DistroVersion::new(Debian, "forky", "14")),
        ("trixie", DistroVersion::new(Debian, "trixie", "13")),
        ("bookworm", DistroVersion::new(Debian, "bookworm", "12")),
        ("bullseye", DistroVersion::new(Debian, "bullseye", "11")),
        ("buster", DistroVersion::new(Debian, "buster", "10")),
        ("stretch", DistroVersion::new(Debian, "stretch", "9")),
        // ubuntu
        (
            "stonking",
            DistroVersion::new(Ubuntu, "stonking", "26.10").devel(),
        ),
        ("resolute", DistroVersion::new(Ubuntu, "resolute", "26.04")),
        ("noble", DistroVersion::new(Ubuntu, "noble", "24.04")),
        ("jammy", DistroVersion::new(Ubuntu, "jammy", "22.04")),
        ("focal", DistroVersion::new(Ubuntu, "focal", "20.04")),
        ("bionic", DistroVersion::new(Ubuntu, "bionic", "18.04")),
        ("xenial", DistroVersion::new(Ubuntu, "xenial", "16.04")),
        ("trusty", DistroVersion::new(Ubuntu, "trusty", "14.04")),
    ])
});

pub fn get_distro_version(codename: &str) -> Option<DistroVersion> {
    DISTRO_INFO_MAP.get(codename).cloned()
}
