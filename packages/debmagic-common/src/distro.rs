use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

/// Distribution family. Known Debian/Ubuntu variants plus an open [`Custom`] name
/// for other apt/dpkg targets (`yocto`, …).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Distro {
    Debian,
    Ubuntu,
    Custom(String),
}

impl Distro {
    /// Parse a family name from config keys, `/etc/os-release` `ID`, or serialized form.
    ///
    /// `"debian"`/`"Debian"` and `"ubuntu"`/`"Ubuntu"` map to the known variants;
    /// anything else is [`Distro::Custom`].
    pub fn parse(name: &str) -> Self {
        match name {
            "debian" | "Debian" => Distro::Debian,
            "ubuntu" | "Ubuntu" => Distro::Ubuntu,
            other => Distro::Custom(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Distro::Debian => "debian",
            Distro::Ubuntu => "ubuntu",
            Distro::Custom(name) => name,
        }
    }
}

impl From<&str> for Distro {
    fn from(name: &str) -> Self {
        Distro::parse(name)
    }
}

impl From<String> for Distro {
    fn from(name: String) -> Self {
        Distro::parse(&name)
    }
}

impl fmt::Display for Distro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Distro {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Distro {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        // Accept legacy PascalCase unit-enum spellings from older build.json.
        Ok(Distro::parse(&s))
    }
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
    pub fn new(distro: Distro, codename: &str, version: &str) -> Self {
        Self {
            distro,
            codename: codename.to_string(),
            version: version.to_string(),
            is_devel: false,
        }
    }

    /// Custom (non-built-in) target: family + codename, empty version.
    pub fn custom(distro: Distro, codename: &str) -> Self {
        Self::new(distro, codename, "")
    }

    fn devel(mut self) -> Self {
        self.is_devel = true;
        self
    }

    pub fn key(&self) -> String {
        format!("{}:{}", self.distro, self.codename)
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
        // Suite alias: sid → unstable (concrete release identity).
        ("sid", DistroVersion::new(Debian, "unstable", "")),
        ("testing", DistroVersion::new(Debian, "testing", "")),
        ("duke", DistroVersion::new(Debian, "duke", "15")),
        ("forky", DistroVersion::new(Debian, "forky", "14")),
        ("trixie", DistroVersion::new(Debian, "trixie", "13")),
        // Suite alias: stable → current stable release (update when Debian rolls).
        ("stable", DistroVersion::new(Debian, "trixie", "13")),
        ("bookworm", DistroVersion::new(Debian, "bookworm", "12")),
        // Suite alias: oldstable → current oldstable release.
        ("oldstable", DistroVersion::new(Debian, "bookworm", "12")),
        ("bullseye", DistroVersion::new(Debian, "bullseye", "11")),
        ("buster", DistroVersion::new(Debian, "buster", "10")),
        ("stretch", DistroVersion::new(Debian, "stretch", "9")),
        // ubuntu
        (
            "stonking",
            DistroVersion::new(Ubuntu, "stonking", "26.10").devel(),
        ),
        // Suite alias: devel → current Ubuntu development release.
        (
            "devel",
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

/// Look up a built-in distribution by codename or suite alias.
///
/// Suite aliases are map keys that resolve to a concrete release [`DistroVersion`]:
/// - Debian: `stable` → current stable release, `oldstable` → current oldstable,
///   `sid` → `unstable`
/// - Ubuntu: `devel` → current development release
///
/// Alias targets are maintained manually when Debian/Ubuntu roll.
/// Non-built-in suites are not returned here; callers resolve those via Driver
/// `base_images` or Bare `/etc/os-release` checks.
pub fn get_distro_version(name: &str) -> Option<DistroVersion> {
    DISTRO_INFO_MAP.get(name).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distro_serde_accepts_legacy_pascal_case() {
        let d: Distro = serde_json::from_str("\"Debian\"").unwrap();
        assert_eq!(d, Distro::Debian);
        assert_eq!(serde_json::to_string(&d).unwrap(), "\"debian\"");
    }

    #[test]
    fn distro_serde_custom_is_plain_string() {
        let d = Distro::Custom("yocto".into());
        assert_eq!(serde_json::to_string(&d).unwrap(), "\"yocto\"");
        let parsed: Distro = serde_json::from_str("\"yocto\"").unwrap();
        assert_eq!(parsed, d);
    }
}
