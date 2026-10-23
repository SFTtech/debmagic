//! What apt needs to know about a target distro: the archive layout a build
//! environment pulls from, and the features of the release's apt.

use std::str::FromStr;

use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::distro::{Distro, DistroVersion};

/// Debian suites without `-updates`/`-security` pockets: unstable, and
/// experimental, an add-on to it.
const DEBIAN_UNSTABLE: &[&str] = &["unstable", "sid", "experimental"];
const DEBIAN_ARCHIVE: &str = "http://deb.debian.org/debian";
/// Debian mirrors don't carry the security pocket.
const DEBIAN_SECURITY_ARCHIVE: &str = "http://deb.debian.org/debian-security";
const UBUNTU_ARCHIVE: &str = "http://archive.ubuntu.com/ubuntu";
const UBUNTU_PORTS_ARCHIVE: &str = "http://ports.ubuntu.com/ubuntu-ports";
const EXPERIMENTAL: &str = "experimental";

/// Which packages a build takes from a suite beyond its release.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SelectionValue", into = "SelectionValue")]
pub enum Selection {
    #[default]
    Off,
    /// Whatever apt prefers with the suite enabled.
    All,
    /// Only these packages: apt preferences patterns like binary package
    /// names, `src:<source>` or globs. Everything else stays with the
    /// release.
    Only(Vec<String>),
}

/// [`Selection`] in config files: a bool or a list of packages.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum SelectionValue {
    Enabled(bool),
    Packages(Vec<String>),
}

impl Selection {
    fn packages(packages: Vec<String>) -> Result<Self, String> {
        if let Some(invalid) = packages
            .iter()
            .find(|package| package.is_empty() || package.contains(char::is_whitespace))
        {
            return Err(format!("invalid package pattern '{invalid}'"));
        }
        Ok(if packages.is_empty() {
            Self::Off
        } else {
            Self::Only(packages)
        })
    }

    pub fn is_on(&self) -> bool {
        *self != Self::Off
    }
}

/// A bool, or a comma-separated list of packages.
impl FromStr for Selection {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "y" | "yes" | "t" | "true" | "on" | "1" => Ok(Self::All),
            "n" | "no" | "f" | "false" | "off" | "0" => Ok(Self::Off),
            _ => Self::packages(value.split(',').map(|p| p.trim().to_string()).collect()),
        }
    }
}

impl TryFrom<SelectionValue> for Selection {
    type Error = String;

    fn try_from(value: SelectionValue) -> Result<Self, String> {
        match value {
            SelectionValue::Enabled(true) => Ok(Self::All),
            SelectionValue::Enabled(false) => Ok(Self::Off),
            SelectionValue::Packages(packages) => Self::packages(packages),
        }
    }
}

impl From<Selection> for SelectionValue {
    fn from(selection: Selection) -> Self {
        match selection {
            Selection::Off => Self::Enabled(false),
            Selection::All => Self::Enabled(true),
            Selection::Only(packages) => Self::Packages(packages),
        }
    }
}

/// The suites beyond its release a build environment takes packages from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Suites {
    /// From the suite the release's updates migrate from.
    pub proposed: Selection,
    /// From Debian experimental.
    pub experimental: Selection,
}

impl Suites {
    /// The suites for a build for `distro`. Experimental only adds to
    /// Debian unstable and testing, and is all of the experimental release.
    pub fn for_distro(
        distro: &DistroVersion,
        proposed: Selection,
        experimental: Selection,
    ) -> anyhow::Result<Self> {
        let experimental = match experimental {
            Selection::Off if is_experimental(distro) => Selection::All,
            Selection::Off => Selection::Off,
            _ if distro.distro == Distro::Debian
                && (DEBIAN_UNSTABLE.contains(&distro.codename.as_str()) || distro.is_devel) =>
            {
                experimental
            }
            _ => bail!(
                "experimental only adds to Debian unstable and testing, not to {} {}",
                distro.distro,
                distro.codename
            ),
        };
        Ok(Self {
            proposed,
            experimental,
        })
    }
}

fn is_experimental(distro: &DistroVersion) -> bool {
    distro.distro == Distro::Debian && distro.codename == EXPERIMENTAL
}

/// A deb822 `.sources` file pointing apt at `mirror` (the distro's default
/// archive if `None`) for the release, its `-updates` and `-security`
/// pockets, plus the opted-in `suites`. It replaces the base image's own
/// sources. `dpkg_arch` is the environment's architecture: Ubuntu serves
/// most of them from a separate ports archive.
pub fn sources_file(
    distro: &DistroVersion,
    mirror: Option<&str>,
    extra: &Suites,
    dpkg_arch: &str,
) -> anyhow::Result<String> {
    let codename = distro.codename.as_str();
    let (archive, components) = match &distro.distro {
        Distro::Debian => (DEBIAN_ARCHIVE, debian_components(&distro.version)),
        Distro::Ubuntu => (
            match dpkg_arch {
                "amd64" | "i386" => UBUNTU_ARCHIVE,
                _ => UBUNTU_PORTS_ARCHIVE,
            },
            &["main", "restricted", "universe", "multiverse"][..],
        ),
        Distro::Custom(family) => {
            bail!("no known apt archive for {family}; configure the base image's sources instead")
        }
    };
    let debian = distro.distro == Distro::Debian;
    let rolling = debian && DEBIAN_UNSTABLE.contains(&codename);
    let security = format!("{codename}-security");
    let mut suites = vec![if is_experimental(distro) {
        "unstable".to_string()
    } else {
        codename.to_string()
    }];
    if !rolling {
        suites.push(format!("{codename}-updates"));
        if !debian {
            suites.push(security.clone());
        }
    }
    if extra.proposed.is_on() {
        suites.extend(proposed_suite(distro));
    }
    if extra.experimental.is_on() {
        suites.push(EXPERIMENTAL.to_string());
    }

    let mut groups = vec![(mirror.unwrap_or(archive), suites)];
    if debian && !rolling {
        groups.push((DEBIAN_SECURITY_ARCHIVE, vec![security]));
    }
    let paragraphs: Vec<String> = groups
        .into_iter()
        .map(|(uri, suites)| {
            format!(
                "Types: deb deb-src\nURIs: {uri}\nSuites: {}\nComponents: {}\n",
                suites.join(" "),
                components.join(" ")
            )
        })
        .collect();
    Ok(paragraphs.join("\n"))
}

fn debian_components(version: &str) -> &'static [&'static str] {
    // rolling suites have no version and are past 12
    match version.parse::<u32>() {
        Ok(major) if major < 12 => &["main", "contrib", "non-free"],
        _ => &["main", "contrib", "non-free", "non-free-firmware"],
    }
}

/// The suite updates of the release migrate from, `None` for unstable
/// itself: unstable migrates into Debian's devel release (testing).
fn proposed_suite(distro: &DistroVersion) -> Option<String> {
    let codename = &distro.codename;
    match distro.distro {
        Distro::Debian if DEBIAN_UNSTABLE.contains(&codename.as_str()) => None,
        Distro::Debian if distro.is_devel => Some("unstable".to_string()),
        Distro::Debian => Some(format!("{codename}-proposed-updates")),
        _ => Some(format!("{codename}-proposed")),
    }
}

/// The NotAutomatic suite a build takes all packages from only when nothing
/// else satisfies its build dependencies: experimental.
pub fn retry_suite(extra: &Suites) -> Option<&'static str> {
    (extra.experimental == Selection::All).then_some(EXPERIMENTAL)
}

/// apt preferences for the `extra` suites of [`sources_file`]. Ubuntu
/// marks `-proposed` NotAutomatic, so apt would only upgrade installed
/// packages from it; pinned to the release's priority like on Launchpad's
/// builders, the build takes everything newer from it. Debian's proposed
/// suites aren't NotAutomatic, so with only some packages from them the
/// rest is pinned down like Ubuntu's.
pub fn preferences_file(distro: &DistroVersion, extra: &Suites) -> Option<String> {
    let mut pins = Vec::new();
    if let Some(suite) = proposed_suite(distro) {
        // Debian's proposed-updates have a Suite like `proposed-updates`
        let release = if suite.ends_with("-proposed-updates") {
            format!("n={suite}")
        } else {
            format!("a={suite}")
        };
        match &extra.proposed {
            Selection::Off => {}
            Selection::All if distro.distro == Distro::Ubuntu => {
                pins.push(pin("*", &release, 500));
            }
            Selection::All => {}
            Selection::Only(packages) => {
                pins.push(pin(&packages.join(" "), &release, 990));
                pins.push(pin("*", &release, 100));
            }
        }
    }
    if let Selection::Only(packages) = &extra.experimental {
        pins.push(pin(&packages.join(" "), &format!("a={EXPERIMENTAL}"), 990));
    }
    (!pins.is_empty()).then(|| pins.join("\n"))
}

fn pin(packages: &str, release: &str, priority: u32) -> String {
    format!("Package: {packages}\nPin: release {release}\nPin-Priority: {priority}\n")
}

/// Whether the release's apt has `apt-get satisfy` (apt 1.9: Debian 11,
/// Ubuntu 20.04). Unknown versions count as recent.
pub fn has_satisfy(distro: &DistroVersion) -> bool {
    let major = distro
        .version
        .split('.')
        .next()
        .and_then(|major| major.parse::<u32>().ok());
    match (&distro.distro, major) {
        (Distro::Debian, Some(major)) => major >= 11,
        (Distro::Ubuntu, Some(major)) => major >= 20,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    fn distro(name: &str) -> DistroVersion {
        let (distro, version) = match name {
            "buster" => (Distro::Debian, "10"),
            "bullseye" => (Distro::Debian, "11"),
            "trixie" => (Distro::Debian, "13"),
            "forky" => (Distro::Debian, "14"),
            "unstable" | "experimental" => (Distro::Debian, ""),
            "bionic" => (Distro::Ubuntu, "18.04"),
            "focal" => (Distro::Ubuntu, "20.04"),
            "noble" => (Distro::Ubuntu, "24.04"),
            other => panic!("no test release {other}"),
        };
        DistroVersion::new(distro, name, version)
    }

    fn suites(distro: &DistroVersion, proposed: &str, experimental: &str) -> Suites {
        let proposed = proposed.parse().unwrap();
        Suites::for_distro(distro, proposed, experimental.parse().unwrap()).unwrap()
    }

    fn proposed() -> Suites {
        Suites {
            proposed: Selection::All,
            ..Suites::default()
        }
    }

    #[test_case("yes", Selection::All)]
    #[test_case("off", Selection::Off)]
    #[test_case("libfoo, src:bar", Selection::Only(vec!["libfoo".into(), "src:bar".into()]))]
    fn selection_from_str(value: &str, expected: Selection) {
        assert_eq!(value.parse::<Selection>(), Ok(expected));
    }

    #[test_case("")]
    #[test_case("libfoo,")]
    #[test_case("lib foo")]
    fn selection_rejects_invalid_packages(value: &str) {
        assert!(value.parse::<Selection>().is_err());
    }

    #[test]
    fn selection_in_config_is_bool_or_packages() {
        let parse = |json| serde_json::from_str::<Selection>(json).unwrap();
        assert_eq!(parse("true"), Selection::All);
        assert_eq!(parse("[]"), Selection::Off);
        assert_eq!(
            parse(r#"["libfoo"]"#),
            Selection::Only(vec!["libfoo".into()])
        );
        assert!(serde_json::from_str::<Selection>(r#"["a b"]"#).is_err());
    }

    #[test_case("noble")]
    #[test_case("trixie")]
    fn experimental_only_adds_to_debian_unstable_and_testing(name: &str) {
        let error = Suites::for_distro(&distro(name), Selection::Off, Selection::All);
        assert!(error.is_err());
    }

    #[test]
    fn experimental_release_is_all_of_experimental() {
        let experimental = suites(&distro("experimental"), "no", "no");
        assert_eq!(experimental.experimental, Selection::All);
        assert_eq!(retry_suite(&experimental), Some("experimental"));
        assert_eq!(
            preferences_file(&distro("experimental"), &experimental),
            None
        );
    }

    #[test]
    fn debian_stable_keeps_security_on_the_debian_archive() {
        let trixie = distro("trixie");
        let sources = sources_file(&trixie, Some("http://m/debian"), &proposed(), "amd64");
        assert_eq!(
            sources.unwrap(),
            "Types: deb deb-src\nURIs: http://m/debian\n\
             Suites: trixie trixie-updates trixie-proposed-updates\n\
             Components: main contrib non-free non-free-firmware\n\
             \n\
             Types: deb deb-src\nURIs: http://deb.debian.org/debian-security\n\
             Suites: trixie-security\n\
             Components: main contrib non-free non-free-firmware\n"
        );
    }

    #[test_case(false ; "release")]
    #[test_case(true ; "proposed is unstable itself")]
    fn debian_unstable_is_one_suite(with_proposed: bool) {
        let extra = if with_proposed {
            proposed()
        } else {
            Suites::default()
        };
        let sources = sources_file(&distro("unstable"), None, &extra, "arm64").unwrap();
        assert_eq!(
            sources,
            "Types: deb deb-src\nURIs: http://deb.debian.org/debian\nSuites: unstable\n\
             Components: main contrib non-free non-free-firmware\n"
        );
    }

    #[test]
    fn debian_devel_proposed_is_unstable() {
        let mut forky = distro("forky");
        forky.is_devel = true;
        let sources = sources_file(&forky, None, &proposed(), "amd64").unwrap();
        assert!(
            sources.contains("Suites: forky forky-updates unstable\n"),
            "{sources}"
        );
        assert!(sources.contains("Suites: forky-security\n"), "{sources}");
        assert_eq!(preferences_file(&forky, &proposed()), None);

        let extra = suites(&forky, "libfoo", "src:bar");
        let sources = sources_file(&forky, None, &extra, "amd64").unwrap();
        assert!(
            sources.contains("Suites: forky forky-updates unstable experimental\n"),
            "{sources}"
        );
        assert_eq!(
            preferences_file(&forky, &extra).unwrap(),
            "Package: libfoo\nPin: release a=unstable\nPin-Priority: 990\n\n\
             Package: *\nPin: release a=unstable\nPin-Priority: 100\n\n\
             Package: src:bar\nPin: release a=experimental\nPin-Priority: 990\n"
        );
        assert_eq!(retry_suite(&extra), None);
    }

    #[test]
    fn debian_proposed_updates_is_pinned_by_codename() {
        let trixie = distro("trixie");
        assert_eq!(
            preferences_file(&trixie, &suites(&trixie, "libfoo", "no")).unwrap(),
            "Package: libfoo\nPin: release n=trixie-proposed-updates\nPin-Priority: 990\n\n\
             Package: *\nPin: release n=trixie-proposed-updates\nPin-Priority: 100\n"
        );
        assert_eq!(preferences_file(&trixie, &proposed()), None);
    }

    #[test]
    fn debian_experimental_adds_to_unstable() {
        let experimental = distro("experimental");
        let sources = sources_file(
            &experimental,
            None,
            &suites(&experimental, "yes", "no"),
            "amd64",
        );
        assert!(
            sources
                .as_ref()
                .unwrap()
                .contains("Suites: unstable experimental\n"),
            "{sources:?}"
        );
        let unstable = distro("unstable");
        let extra = suites(&unstable, "no", "yes");
        let sources = sources_file(&unstable, None, &extra, "amd64").unwrap();
        assert!(
            sources.contains("Suites: unstable experimental\n"),
            "{sources}"
        );
        assert_eq!(retry_suite(&extra), Some("experimental"));
    }

    #[test]
    fn ubuntu_proposed_is_pinned_like_the_release() {
        let noble = distro("noble");
        assert_eq!(
            preferences_file(&noble, &proposed()).unwrap(),
            "Package: *\nPin: release a=noble-proposed\nPin-Priority: 500\n"
        );
        assert_eq!(preferences_file(&noble, &Suites::default()), None);
    }

    #[test]
    fn old_debian_has_no_firmware_component() {
        let extra = Suites::default();
        let sources = sources_file(&distro("bullseye"), None, &extra, "amd64").unwrap();
        assert!(
            sources.contains("Components: main contrib non-free\n"),
            "{sources}"
        );
    }

    #[test_case("amd64", UBUNTU_ARCHIVE ; "archive")]
    #[test_case("arm64", UBUNTU_PORTS_ARCHIVE ; "ports")]
    fn ubuntu_proposed_uses_the_default_archive(arch: &str, archive: &str) {
        let sources = sources_file(&distro("noble"), None, &proposed(), arch).unwrap();
        assert_eq!(
            sources,
            format!(
                "Types: deb deb-src\nURIs: {archive}\n\
                 Suites: noble noble-updates noble-security noble-proposed\n\
                 Components: main restricted universe multiverse\n"
            )
        );
    }

    #[test]
    fn custom_distro_has_no_known_archive() {
        let custom = DistroVersion::custom(Distro::Custom("yocto".into()), "scarthgap");
        assert!(sources_file(&custom, Some("http://m"), &Suites::default(), "amd64").is_err());
    }

    #[test_case("buster", false)]
    #[test_case("bullseye", true)]
    #[test_case("unstable", true)]
    #[test_case("bionic", false)]
    #[test_case("focal", true)]
    fn test_has_satisfy(name: &str, expected: bool) {
        assert_eq!(has_satisfy(&distro(name)), expected);
    }
}
