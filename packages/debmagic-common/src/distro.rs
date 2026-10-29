use anyhow::Context;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

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

    pub fn key(&self) -> String {
        format!("{}:{}", self.distro, self.codename)
    }
}

/// The known Debian and Ubuntu releases and suite aliases, from
/// distro-info-data's `debian.csv` and `ubuntu.csv`.
///
/// A release's [`DistroVersion::codename`] is the CSV's `series` (`trixie`);
/// its `codename` column (`Trixie`) is only a display name.
#[derive(Debug, Clone, Default)]
pub struct DistroTable {
    by_name: HashMap<String, DistroVersion>,
    /// Families without a release in development: their data predates the
    /// latest release, like python-distro-info's `DistroDataOutdated`.
    outdated: Vec<Distro>,
}

impl DistroTable {
    /// The table as of `today`, which decides the suite aliases: Debian's
    /// `stable` and `oldstable` are the last two releases, `testing` the
    /// one in development (created, not released yet), as is Ubuntu's
    /// `devel`. `sid` is `unstable`.
    pub fn from_distro_info(
        debian_csv: &str,
        ubuntu_csv: &str,
        today: NaiveDate,
    ) -> anyhow::Result<Self> {
        let mut table = Self::default();
        let debian = parse_distro_info(debian_csv).context("parsing debian.csv failed")?;
        table.add(Distro::Debian, &debian, today);
        let ubuntu = parse_distro_info(ubuntu_csv).context("parsing ubuntu.csv failed")?;
        table.add(Distro::Ubuntu, &ubuntu, today);
        Ok(table)
    }

    fn add(&mut self, distro: Distro, releases: &[Release], today: NaiveDate) {
        let (devel_alias, released_aliases) = match distro {
            Distro::Debian => ("testing", &["stable", "oldstable"][..]),
            _ => ("devel", &[][..]),
        };
        let mut released = Vec::new();
        let mut has_devel = false;
        for release in releases {
            // unstable and experimental have no version
            let numbered = !release.version.is_empty();
            let is_released = release.release.is_some_and(|date| date <= today);
            let version = DistroVersion {
                distro: distro.clone(),
                codename: match (&distro, release.series.as_str()) {
                    (Distro::Debian, "sid") => "unstable".to_string(),
                    (_, series) => series.to_string(),
                },
                version: release.version.clone(),
                is_devel: numbered && release.created <= today && !is_released,
            };
            if version.is_devel {
                has_devel = true;
                self.insert(devel_alias, &version);
            }
            if numbered && is_released {
                released.push(version.clone());
            }
            self.insert(&release.series, &version);
            self.insert(&version.codename, &version);
        }
        for (alias, version) in released_aliases.iter().zip(released.iter().rev()) {
            self.insert(alias, version);
        }
        if !has_devel {
            self.outdated.push(distro);
        }
    }

    fn insert(&mut self, name: &str, version: &DistroVersion) {
        self.by_name.insert(name.to_string(), version.clone());
    }

    /// The families whose data is older than their latest release, so it
    /// lacks the release now in development.
    pub fn outdated(&self) -> &[Distro] {
        &self.outdated
    }

    /// Look up a release by codename or suite alias.
    ///
    /// A pocket suffix (`resolute-security`, `bookworm-backports`, ...)
    /// resolves to its release: the build environment is the same.
    /// Other suites are not returned here; callers resolve those via Driver
    /// `base_images` or Bare `/etc/os-release` checks.
    pub fn get(&self, name: &str) -> Option<DistroVersion> {
        self.by_name
            .get(name)
            .or_else(|| self.by_name.get(strip_pocket(name)?))
            .cloned()
    }
}

/// One row of a distro-info-data CSV.
struct Release {
    /// `26.04` without the ` LTS` suffix, empty for Debian's unstable and
    /// experimental.
    version: String,
    series: String,
    created: NaiveDate,
    release: Option<NaiveDate>,
}

fn parse_distro_info(csv: &str) -> anyhow::Result<Vec<Release>> {
    let mut lines = csv.lines().filter(|line| !line.trim().is_empty());
    let header: Vec<&str> = lines.next().context("no header")?.split(',').collect();
    let column = |name: &str| {
        header
            .iter()
            .position(|column| *column == name)
            .with_context(|| format!("no {name} column"))
    };
    let [version, series, created, release] = [
        column("version")?,
        column("series")?,
        column("created")?,
        column("release")?,
    ];
    let date = |field: &str| NaiveDate::parse_from_str(field, "%Y-%m-%d");
    lines
        .enumerate()
        .map(|(index, line)| {
            let fields: Vec<&str> = line.split(',').collect();
            let field = |column: usize| fields.get(column).copied().unwrap_or_default();
            let release = (|| {
                Ok::<_, anyhow::Error>(Release {
                    version: field(version)
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_string(),
                    series: field(series).to_string(),
                    created: date(field(created)).context("invalid created date")?,
                    release: match field(release) {
                        "" => None,
                        release => Some(date(release).context("invalid release date")?),
                    },
                })
            })();
            release.with_context(|| format!("line {} is invalid: {line}", index + 2))
        })
        .collect()
}

/// Pocket suffixes of Debian and Ubuntu suites
const POCKETS: &[&str] = &["security", "updates", "proposed", "backports"];

/// The release part of a `<release>-<pocket>` suite name
fn strip_pocket(name: &str) -> Option<&str> {
    let (release, pocket) = name.rsplit_once('-')?;
    POCKETS.contains(&pocket).then_some(release)
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

    const DEBIAN: &str = "version,codename,series,created,release,eol\n\
        12,Bookworm,bookworm,2021-08-14,2023-06-10,2026-07-11\n\
        13,Trixie,trixie,2023-06-10,2025-08-09\n\
        14,Forky,forky,2025-08-09\n\
        15,Duke,duke,2027-08-01\n\
        ,Sid,sid,1993-08-16\n\
        ,Experimental,experimental,1993-08-16\n";
    const UBUNTU: &str = "version,codename,series,created,release,eol\n\
        26.04 LTS,Resolute Raccoon,resolute,2025-10-09,2026-04-23,2031-05-29\n\
        26.10,Stonking Stingray,stonking,2026-04-24,2026-10-15,2027-07-15\n\
        27.04,Tenacious T-Rex,tenacious,2026-10-16,2027-04-22\n";

    fn table(year: i32, month: u32, day: u32) -> DistroTable {
        let today = NaiveDate::from_ymd_opt(year, month, day).unwrap();
        DistroTable::from_distro_info(DEBIAN, UBUNTU, today).unwrap()
    }

    #[test]
    fn pocket_suffix_resolves_to_release() {
        let table = table(2026, 10, 23);
        let resolute = table.get("resolute");
        assert!(resolute.is_some());
        for suite in ["resolute-security", "resolute-updates", "resolute-proposed"] {
            assert_eq!(table.get(suite), resolute, "{suite}");
        }
        assert_eq!(table.get("bookworm-backports"), table.get("bookworm"));
        assert_eq!(table.get("resolute-nope"), None);
        assert_eq!(table.get("yocto-security"), None);
    }

    #[test_case::test_case("stable", "trixie", "13", false)]
    #[test_case::test_case("oldstable", "bookworm", "12", false)]
    #[test_case::test_case("testing", "forky", "14", true)]
    #[test_case::test_case("forky", "forky", "14", true)]
    #[test_case::test_case("duke", "duke", "15", false ; "not created yet")]
    #[test_case::test_case("sid", "unstable", "", false)]
    #[test_case::test_case("unstable", "unstable", "", false)]
    #[test_case::test_case("experimental", "experimental", "", false)]
    #[test_case::test_case("devel", "tenacious", "27.04", true)]
    #[test_case::test_case("stonking", "stonking", "26.10", false ; "just released")]
    #[test_case::test_case("resolute", "resolute", "26.04", false ; "lts suffix")]
    fn table_resolves(name: &str, codename: &str, version: &str, is_devel: bool) {
        let resolved = table(2026, 10, 23).get(name).unwrap();
        assert_eq!(
            (
                resolved.codename.as_str(),
                resolved.version.as_str(),
                resolved.is_devel
            ),
            (codename, version, is_devel)
        );
    }

    #[test]
    fn aliases_follow_the_date() {
        let before_release = table(2026, 10, 14);
        assert_eq!(before_release.get("devel").unwrap().codename, "stonking");
        assert!(before_release.get("stonking").unwrap().is_devel);
        let old = table(2024, 1, 1);
        assert_eq!(old.get("stable").unwrap().codename, "bookworm");
        assert_eq!(old.get("testing").unwrap().codename, "trixie");
    }

    #[test]
    fn data_without_a_release_in_development_is_outdated() {
        assert_eq!(table(2026, 10, 23).outdated(), &[]);
        // after tenacious and before duke was created
        assert_eq!(table(2027, 5, 1).outdated(), &[Distro::Ubuntu]);
    }

    #[test]
    fn invalid_distro_info_names_the_line() {
        let error = DistroTable::from_distro_info(
            "version,codename,series,created,release\n13,Trixie,trixie,soon,\n",
            UBUNTU,
            NaiveDate::from_ymd_opt(2026, 10, 23).unwrap(),
        )
        .unwrap_err();
        assert!(
            format!("{error:#}").contains("line 2 is invalid"),
            "{error:#}"
        );
    }

    #[test]
    fn distro_serde_custom_is_plain_string() {
        let d = Distro::Custom("yocto".into());
        assert_eq!(serde_json::to_string(&d).unwrap(), "\"yocto\"");
        let parsed: Distro = serde_json::from_str("\"yocto\"").unwrap();
        assert_eq!(parsed, d);
    }
}
