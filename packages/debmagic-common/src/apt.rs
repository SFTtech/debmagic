//! What apt needs to know about a target distro: the archive layout a build
//! environment pulls from, and the features of the release's apt.

use std::{collections::BTreeMap, str::FromStr};

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
const RELEASE: &str = "release";
const PROPOSED: &str = "proposed";
const EXPERIMENTAL: &str = "experimental";
/// The archives a package can be taken from.
const ARCHIVES: &[&str] = &[RELEASE, PROPOSED, EXPERIMENTAL];

/// How a build environment installs the packages matching an apt
/// preferences pattern: a binary package name, `src:<source>` for all its
/// binaries, or a glob.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PackageSettings {
    /// The archive to take it from: `release` (with its updates),
    /// `proposed` or `experimental`.
    pub from: Option<String>,
    /// The version to install, even if older, from whichever archive in use
    /// has it.
    pub version: Option<String>,
}

/// [`PackageSettings`] by pattern.
pub type Packages = BTreeMap<String, PackageSettings>;

/// A `<pattern>[=<version>][/<archive>]` package spec, apt-get's syntax.
pub fn parse_package(spec: &str) -> Result<(String, PackageSettings), String> {
    let (package, from) = match spec.split_once('/') {
        Some((package, from)) => (package, Some(from.to_string())),
        None => (spec, None),
    };
    let (pattern, version) = match package.split_once('=') {
        Some((pattern, version)) => (pattern, Some(version.to_string())),
        None => (package, None),
    };
    validate_pattern(pattern)?;
    let settings = PackageSettings { from, version };
    if settings == PackageSettings::default() {
        return Err(format!(
            "'{spec}' needs a version (=<version>) or an archive (/<archive>)"
        ));
    }
    if let Some(invalid) = [&settings.from, &settings.version]
        .into_iter()
        .flatten()
        .find(|value| value.is_empty() || value.contains(char::is_whitespace))
    {
        return Err(format!("invalid '{invalid}' in '{spec}'"));
    }
    Ok((pattern.to_string(), settings))
}

fn validate_pattern(pattern: &str) -> Result<(), String> {
    match pattern.is_empty() || pattern.contains(char::is_whitespace) {
        true => Err(format!("invalid package pattern '{pattern}'")),
        false => Ok(()),
    }
}

/// Whether `pattern` names the source package `source` or one of its
/// `binaries`, globs aside.
pub fn names_package(pattern: &str, source: &str, binaries: &[String]) -> bool {
    match pattern.strip_prefix("src:") {
        Some(name) => without_arch(name) == source,
        None => binaries
            .iter()
            .any(|binary| binary == without_arch(pattern)),
    }
}

fn without_arch(package: &str) -> &str {
    package.split_once(':').map_or(package, |(name, _)| name)
}

/// `--proposed`/`--experimental`: a bool, or a comma-separated list of
/// packages to take from that suite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    Off,
    /// Whatever apt prefers with the suite enabled.
    All,
    Only(Vec<String>),
}

impl FromStr for Selection {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "y" | "yes" | "t" | "true" | "on" | "1" => Ok(Self::All),
            "n" | "no" | "f" | "false" | "off" | "0" => Ok(Self::Off),
            _ => {
                let packages: Vec<String> =
                    value.split(',').map(|p| p.trim().to_string()).collect();
                for package in &packages {
                    validate_pattern(package)?;
                }
                Ok(Self::Only(packages))
            }
        }
    }
}

/// The archives beyond its release a build environment takes packages
/// from, and how it installs some packages.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Archives {
    /// All of the proposed suite, as apt prefers it.
    pub proposed: bool,
    /// All of experimental, for what nothing else provides.
    pub experimental: bool,
    pub packages: Packages,
}

impl Archives {
    /// The archives for a build for `distro`: all of experimental for the
    /// experimental release, which only adds to unstable.
    pub fn for_distro(
        distro: &DistroVersion,
        proposed: bool,
        experimental: bool,
        packages: Packages,
    ) -> anyhow::Result<Self> {
        let archives = Self {
            proposed,
            experimental: experimental || is_experimental(distro),
            packages,
        };
        let release = format!("{} {}", distro.distro, distro.codename);
        for (pattern, settings) in &archives.packages {
            match settings.from.as_deref() {
                Some(from) if !ARCHIVES.contains(&from) => bail!(
                    "unknown archive '{from}' for {pattern}, known are {}",
                    ARCHIVES.join(", ")
                ),
                Some(PROPOSED) if proposed_suite(distro).is_none() => {
                    bail!("{release} has no proposed suite to take {pattern} from")
                }
                _ => {}
            }
        }
        let adds_to_experimental = distro.distro == Distro::Debian
            && (DEBIAN_UNSTABLE.contains(&distro.codename.as_str()) || distro.is_devel);
        if archives.uses(EXPERIMENTAL) && !adds_to_experimental {
            bail!("experimental only adds to Debian unstable and testing, not to {release}");
        }
        Ok(archives)
    }

    /// Whether it takes packages from `archive`.
    fn uses(&self, archive: &str) -> bool {
        let all = match archive {
            PROPOSED => self.proposed,
            EXPERIMENTAL => self.experimental,
            _ => false,
        };
        all || self
            .packages
            .values()
            .any(|settings| settings.from.as_deref() == Some(archive))
    }

    /// Whether it adds suites to the release's.
    pub fn adds_suites(&self) -> bool {
        self.uses(PROPOSED) || self.uses(EXPERIMENTAL)
    }
}

fn is_experimental(distro: &DistroVersion) -> bool {
    distro.distro == Distro::Debian && distro.codename == EXPERIMENTAL
}

/// A deb822 `.sources` file pointing apt at `mirror` (the distro's default
/// archive if `None`) for the release, its `-updates` and `-security`
/// pockets, plus the suites of `archives`. It replaces the base image's own
/// sources. `dpkg_arch` is the environment's architecture: Ubuntu serves
/// most of them from a separate ports archive.
pub fn sources_file(
    distro: &DistroVersion,
    mirror: Option<&str>,
    archives: &Archives,
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
    let release = release_suites(distro);
    let security = format!("{codename}-security");
    let mut suites: Vec<String> = release
        .iter()
        .filter(|suite| !debian || **suite != security)
        .cloned()
        .collect();
    if archives.uses(PROPOSED) {
        suites.extend(proposed_suite(distro));
    }
    if archives.uses(EXPERIMENTAL) {
        suites.push(EXPERIMENTAL.to_string());
    }

    let mut groups = vec![(mirror.unwrap_or(archive), suites)];
    if debian && release.contains(&security) {
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

/// The suites of the release: its pockets, or unstable for Debian's
/// rolling ones.
fn release_suites(distro: &DistroVersion) -> Vec<String> {
    let codename = distro.codename.as_str();
    if distro.distro == Distro::Debian && DEBIAN_UNSTABLE.contains(&codename) {
        let base = if is_experimental(distro) {
            "unstable"
        } else {
            codename
        };
        return vec![base.to_string()];
    }
    ["", "-updates", "-security"]
        .map(|pocket| format!("{codename}{pocket}"))
        .to_vec()
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
pub fn retry_suite(archives: &Archives) -> Option<&'static str> {
    archives.experimental.then_some(EXPERIMENTAL)
}

/// apt preferences for the `archives` of [`sources_file`]:
/// - a package with a version gets it, even if older;
/// - one from an archive gets that archive's;
/// - Ubuntu marks `-proposed` NotAutomatic, so apt would only upgrade
///   installed packages from it: with all of it, it's pinned to the
///   release's priority like on Launchpad's builders, so the build takes
///   everything newer from it;
/// - with only some packages from it, the rest of a proposed suite only
///   upgrades installed packages, also on Debian, which doesn't mark them
///   NotAutomatic.
pub fn preferences_file(distro: &DistroVersion, archives: &Archives) -> Option<String> {
    let mut pins = Vec::new();
    for (pattern, settings) in &archives.packages {
        match (&settings.version, &settings.from) {
            (Some(version), _) => pins.push(pin(pattern, &format!("version {version}"), 1001)),
            (None, Some(from)) => {
                for suite in archive_suites(distro, from) {
                    pins.push(pin(pattern, &release_pin(distro, &suite), 990));
                }
            }
            (None, None) => {}
        }
    }
    if let Some(suite) = proposed_suite(distro) {
        let release = release_pin(distro, &suite);
        if archives.proposed {
            if distro.distro == Distro::Ubuntu {
                pins.push(pin("*", &release, 500));
            }
        } else if archives.uses(PROPOSED) {
            pins.push(pin("*", &release, 100));
        }
    }
    (!pins.is_empty()).then(|| pins.join("\n"))
}

fn archive_suites(distro: &DistroVersion, archive: &str) -> Vec<String> {
    match archive {
        PROPOSED => proposed_suite(distro).into_iter().collect(),
        EXPERIMENTAL => vec![EXPERIMENTAL.to_string()],
        _ => release_suites(distro),
    }
}

/// The apt preferences pin of `suite`. Ubuntu's Release files name the
/// pocket in their Suite, while their Codename is the release's. Debian's
/// name it in their Codename, while their Suite is an alias like
/// `stable-updates`, except for unstable (`sid`) and experimental
/// (`rc-buggy`).
fn release_pin(distro: &DistroVersion, suite: &str) -> String {
    if distro.distro == Distro::Ubuntu || DEBIAN_UNSTABLE.contains(&suite) {
        format!("release a={suite}")
    } else {
        format!("release n={suite}")
    }
}

fn pin(packages: &str, pin: &str, priority: u32) -> String {
    format!("Package: {packages}\nPin: {pin}\nPin-Priority: {priority}\n")
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

    /// The archives for `distro` with all of proposed if `proposed`, and
    /// the package `specs`.
    fn archives(distro: &DistroVersion, proposed: bool, specs: &[&str]) -> Archives {
        let packages = specs.iter().map(|spec| parse_package(spec).unwrap());
        Archives::for_distro(distro, proposed, false, packages.collect()).unwrap()
    }

    fn proposed() -> Archives {
        Archives {
            proposed: true,
            ..Archives::default()
        }
    }

    #[test_case("libfoo1/proposed", "libfoo1", None, Some("proposed") ; "archive")]
    #[test_case("src:foo=1:2.0-1", "src:foo", Some("1:2.0-1"), None ; "version with epoch")]
    #[test_case("libfoo1:amd64=2.0/release", "libfoo1:amd64", Some("2.0"), Some("release") ; "both")]
    fn package_spec(spec: &str, pattern: &str, version: Option<&str>, from: Option<&str>) {
        let settings = PackageSettings {
            from: from.map(str::to_string),
            version: version.map(str::to_string),
        };
        assert_eq!(parse_package(spec), Ok((pattern.to_string(), settings)));
    }

    #[test_case("libfoo1" ; "no settings")]
    #[test_case("=1.0" ; "no pattern")]
    #[test_case("libfoo1/" ; "empty archive")]
    #[test_case("lib foo/proposed" ; "whitespace")]
    fn package_spec_rejects(spec: &str) {
        assert!(parse_package(spec).is_err());
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

    #[test_case("libfoo1:amd64", true ; "binary")]
    #[test_case("src:foo:any", true ; "source")]
    #[test_case("libfoo-dev", false ; "other binary")]
    #[test_case("src:food", false ; "other source")]
    fn test_names_package(pattern: &str, expected: bool) {
        assert_eq!(names_package(pattern, "foo", &["libfoo1".into()]), expected);
    }

    #[test]
    fn settings_in_config_reject_unknown_keys() {
        let parse = |json| serde_json::from_str::<PackageSettings>(json);
        assert_eq!(
            parse(r#"{"from": "proposed"}"#).unwrap().from.as_deref(),
            Some("proposed")
        );
        assert!(parse(r#"{"form": "proposed"}"#).is_err());
    }

    #[test_case("noble", "src:foo/experimental" ; "ubuntu")]
    #[test_case("trixie", "src:foo/experimental" ; "debian stable")]
    #[test_case("unstable", "src:foo/proposed" ; "proposed of unstable")]
    #[test_case("noble", "src:foo/backports" ; "unknown archive")]
    fn archives_reject(name: &str, spec: &str) {
        let packages = [parse_package(spec).unwrap()].into();
        assert!(Archives::for_distro(&distro(name), false, false, packages).is_err());
    }

    #[test]
    fn experimental_only_adds_to_debian_unstable_and_testing() {
        assert!(Archives::for_distro(&distro("noble"), false, true, Packages::new()).is_err());
    }

    #[test]
    fn experimental_release_is_all_of_experimental() {
        let experimental = archives(&distro("experimental"), false, &[]);
        assert!(experimental.experimental);
        assert_eq!(retry_suite(&experimental), Some("experimental"));
        assert_eq!(
            preferences_file(&distro("experimental"), &experimental),
            None
        );
    }

    #[test]
    fn ubuntu_package_from_the_release_is_pinned_to_all_its_pockets() {
        let noble = distro("noble");
        let extra = archives(&noble, true, &["src:foo/release", "libbar1=2.0"]);
        assert_eq!(
            preferences_file(&noble, &extra).unwrap(),
            "Package: libbar1\nPin: version 2.0\nPin-Priority: 1001\n\n\
             Package: src:foo\nPin: release a=noble\nPin-Priority: 990\n\n\
             Package: src:foo\nPin: release a=noble-updates\nPin-Priority: 990\n\n\
             Package: src:foo\nPin: release a=noble-security\nPin-Priority: 990\n\n\
             Package: *\nPin: release a=noble-proposed\nPin-Priority: 500\n"
        );
        let release_only = archives(&noble, false, &["src:foo/release"]);
        assert!(!release_only.adds_suites());
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
            Archives::default()
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

        let extra = archives(&forky, false, &["libfoo/proposed", "src:bar/experimental"]);
        let sources = sources_file(&forky, None, &extra, "amd64").unwrap();
        assert!(
            sources.contains("Suites: forky forky-updates unstable experimental\n"),
            "{sources}"
        );
        assert_eq!(
            preferences_file(&forky, &extra).unwrap(),
            "Package: libfoo\nPin: release a=unstable\nPin-Priority: 990\n\n\
             Package: src:bar\nPin: release a=experimental\nPin-Priority: 990\n\n\
             Package: *\nPin: release a=unstable\nPin-Priority: 100\n"
        );
        assert_eq!(retry_suite(&extra), None);
    }

    #[test]
    fn debian_proposed_updates_is_pinned_by_codename() {
        let trixie = distro("trixie");
        assert_eq!(
            preferences_file(&trixie, &archives(&trixie, false, &["libfoo/proposed"])).unwrap(),
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
            &archives(&experimental, true, &[]),
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
        let extra = Archives::for_distro(&unstable, false, true, Packages::new()).unwrap();
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
        assert_eq!(preferences_file(&noble, &Archives::default()), None);
    }

    #[test]
    fn old_debian_has_no_firmware_component() {
        let extra = Archives::default();
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
        assert!(sources_file(&custom, Some("http://m"), &Archives::default(), "amd64").is_err());
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
