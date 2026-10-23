//! What apt needs to know about a target distro: the archive layout a build
//! environment pulls from, and the features of the release's apt.

use anyhow::bail;

use crate::distro::{Distro, DistroVersion};

/// Debian suites without `-updates`/`-security` pockets: unstable, and
/// experimental, an add-on to it.
const DEBIAN_UNSTABLE: &[&str] = &["unstable", "sid", "experimental"];
const DEBIAN_ARCHIVE: &str = "http://deb.debian.org/debian";
/// Debian mirrors don't carry the security pocket.
const DEBIAN_SECURITY_ARCHIVE: &str = "http://deb.debian.org/debian-security";
const UBUNTU_ARCHIVE: &str = "http://archive.ubuntu.com/ubuntu";
const UBUNTU_PORTS_ARCHIVE: &str = "http://ports.ubuntu.com/ubuntu-ports";

/// A deb822 `.sources` file pointing apt at `mirror` (the distro's default
/// archive if `None`) for the release, its `-updates` and `-security`
/// pockets, plus the suite its updates migrate from if `proposed`. It
/// replaces the base image's own sources. `dpkg_arch` is the environment's
/// architecture: Ubuntu serves most of them from a separate ports archive.
pub fn sources_file(
    distro: &DistroVersion,
    mirror: Option<&str>,
    proposed: bool,
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
    let mut suites = Vec::new();
    if not_automatic_suite(distro).is_some() {
        suites.push("unstable".to_string());
    }
    suites.push(codename.to_string());
    if !rolling {
        suites.push(format!("{codename}-updates"));
        if !debian {
            suites.push(security.clone());
        }
    }
    if proposed {
        suites.extend(proposed_suite(distro));
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

/// The NotAutomatic suite a build for the release takes packages from only
/// when nothing else satisfies its build dependencies: experimental, which
/// only adds to unstable.
pub fn not_automatic_suite(distro: &DistroVersion) -> Option<&'static str> {
    (distro.distro == Distro::Debian && distro.codename == "experimental").then_some("experimental")
}

/// Whether a build for the release needs [`sources_file`] whatever the
/// mirror: the base images of experimental lack its suite.
pub fn needs_own_sources(distro: &DistroVersion) -> bool {
    not_automatic_suite(distro).is_some()
}

/// apt preferences for the sources of [`sources_file`]. Ubuntu marks
/// `-proposed` NotAutomatic, so apt would only upgrade installed packages
/// from it; pinned to the release's priority like on Launchpad's builders,
/// the build takes everything newer from it. Debian's proposed suites
/// aren't NotAutomatic.
pub fn preferences_file(distro: &DistroVersion, proposed: bool) -> Option<String> {
    (proposed && distro.distro == Distro::Ubuntu).then(|| {
        format!(
            "Package: *\nPin: release a={}-proposed\nPin-Priority: 500\n",
            distro.codename
        )
    })
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

    #[test]
    fn debian_stable_keeps_security_on_the_debian_archive() {
        let sources = sources_file(&distro("trixie"), Some("http://m/debian"), true, "amd64");
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
    fn debian_unstable_is_one_suite(proposed: bool) {
        let sources = sources_file(&distro("unstable"), None, proposed, "arm64").unwrap();
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
        let sources = sources_file(&forky, None, true, "amd64").unwrap();
        assert!(
            sources.contains("Suites: forky forky-updates unstable\n"),
            "{sources}"
        );
        assert!(sources.contains("Suites: forky-security\n"), "{sources}");
        assert_eq!(preferences_file(&forky, true), None);
    }

    #[test]
    fn debian_experimental_adds_to_unstable() {
        let experimental = distro("experimental");
        assert!(needs_own_sources(&experimental));
        assert!(!needs_own_sources(&distro("unstable")));
        assert_eq!(not_automatic_suite(&experimental), Some("experimental"));
        let sources = sources_file(&experimental, None, true, "amd64").unwrap();
        assert!(
            sources.contains("Suites: unstable experimental\n"),
            "{sources}"
        );
    }

    #[test]
    fn ubuntu_proposed_is_pinned_like_the_release() {
        assert_eq!(
            preferences_file(&distro("noble"), true).unwrap(),
            "Package: *\nPin: release a=noble-proposed\nPin-Priority: 500\n"
        );
        assert_eq!(preferences_file(&distro("noble"), false), None);
    }

    #[test]
    fn old_debian_has_no_firmware_component() {
        let sources = sources_file(&distro("bullseye"), None, false, "amd64").unwrap();
        assert!(
            sources.contains("Components: main contrib non-free\n"),
            "{sources}"
        );
    }

    #[test_case("amd64", UBUNTU_ARCHIVE ; "archive")]
    #[test_case("arm64", UBUNTU_PORTS_ARCHIVE ; "ports")]
    fn ubuntu_proposed_uses_the_default_archive(arch: &str, archive: &str) {
        let sources = sources_file(&distro("noble"), None, true, arch).unwrap();
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
        assert!(sources_file(&custom, Some("http://m"), false, "amd64").is_err());
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
