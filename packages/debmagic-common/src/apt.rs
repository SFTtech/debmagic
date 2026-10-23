//! What apt needs to know about a target distro: the archive layout a build
//! environment pulls from, and the features of the release's apt.

use anyhow::bail;

use crate::distro::{Distro, DistroVersion};

/// Debian suites without `-updates`/`-security` pockets.
const DEBIAN_UNSTABLE: &[&str] = &["unstable", "sid", "experimental"];
const DEBIAN_ARCHIVE: &str = "http://deb.debian.org/debian";
/// Debian mirrors don't carry the security pocket.
const DEBIAN_SECURITY_ARCHIVE: &str = "http://deb.debian.org/debian-security";
const UBUNTU_ARCHIVE: &str = "http://archive.ubuntu.com/ubuntu";
const UBUNTU_PORTS_ARCHIVE: &str = "http://ports.ubuntu.com/ubuntu-ports";

/// A deb822 `.sources` file pointing apt at `mirror` (the distro's default
/// archive if `None`) for the release, its `-updates` and `-security`
/// pockets, plus its proposed pocket if `proposed`. It
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
    let mut suites = vec![codename.to_string()];
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

/// The release's proposed pocket, `None` for unstable, which has none.
fn proposed_suite(distro: &DistroVersion) -> Option<String> {
    let codename = &distro.codename;
    match distro.distro {
        Distro::Debian if DEBIAN_UNSTABLE.contains(&codename.as_str()) => None,
        Distro::Debian => Some(format!("{codename}-proposed-updates")),
        _ => Some(format!("{codename}-proposed")),
    }
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
            "unstable" => (Distro::Debian, ""),
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
    #[test_case(true ; "no proposed pocket")]
    fn debian_unstable_is_one_suite(proposed: bool) {
        let sources = sources_file(&distro("unstable"), None, proposed, "arm64").unwrap();
        assert_eq!(
            sources,
            "Types: deb deb-src\nURIs: http://deb.debian.org/debian\nSuites: unstable\n\
             Components: main contrib non-free non-free-firmware\n"
        );
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
