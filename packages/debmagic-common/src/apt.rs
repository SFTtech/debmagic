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

/// An extra archive as `--archive` and `apt.archives` take it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArchiveSpec {
    Ppa(Ppa),
    Deb(DebLine),
}

impl FromStr for ArchiveSpec {
    type Err = String;

    fn from_str(spec: &str) -> Result<Self, String> {
        match spec.starts_with("ppa:") {
            true => spec.parse().map(Self::Ppa),
            false => spec.parse().map(Self::Deb),
        }
    }
}

/// A sources.list(5) one-line entry,
/// `deb [<option>=<value> ...] <uri> <suite> [<component> ...]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebLine {
    /// `deb` or `deb-src`.
    pub kind: String,
    /// By one-line name, like `arch` or `signed-by`.
    pub options: Vec<(String, String)>,
    pub uri: String,
    pub suite: String,
    pub components: Vec<String>,
}

impl FromStr for DebLine {
    type Err = String;

    fn from_str(spec: &str) -> Result<Self, String> {
        let invalid = |why: &str| {
            format!(
                "'{spec}' is neither ppa:<owner>/<name>[@<suite>] nor \
                 deb [<option>=<value> ...] <uri> <suite> [<component> ...]: {why}"
            )
        };
        let (kind, rest) = spec
            .trim()
            .split_once(char::is_whitespace)
            .filter(|(kind, _)| ["deb", "deb-src"].contains(kind))
            .ok_or_else(|| invalid("no deb or deb-src"))?;
        let rest = rest.trim_start();
        let (options, rest) = match rest.strip_prefix('[') {
            Some(rest) => rest.split_once(']').ok_or_else(|| invalid("no ]"))?,
            None => ("", rest),
        };
        let options = options
            .split_whitespace()
            .map(|option| match option.split_once('=') {
                Some((key, value)) if !key.is_empty() && !value.is_empty() => {
                    Ok((key.to_ascii_lowercase(), value.to_string()))
                }
                _ => Err(invalid(&format!("no <option>=<value> in {option}"))),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut words = rest.split_whitespace().map(str::to_string);
        let (Some(uri), Some(suite)) = (words.next(), words.next()) else {
            return Err(invalid("no uri and suite"));
        };
        let components: Vec<String> = words.collect();
        if suite.ends_with('/') != components.is_empty() {
            return Err(invalid(
                "components go with a suite, not with a path ending in /",
            ));
        }
        let line = Self {
            kind: kind.to_string(),
            options,
            uri,
            suite,
            components,
        };
        if let Some(path) = line.option("signed-by")
            && path.starts_with('/')
            && path.contains([',', ' '])
        {
            return Err(invalid("signed-by takes one key file"));
        }
        Ok(line)
    }
}

impl DebLine {
    fn option(&self, key: &str) -> Option<&str> {
        self.options
            .iter()
            .find_map(|(k, value)| (k == key).then_some(value.as_str()))
    }

    /// The key file of its `signed-by`, unless it lists fingerprints.
    pub fn signed_by_file(&self) -> Option<&str> {
        self.option("signed-by").filter(|by| by.starts_with('/'))
    }

    /// It as the archive `name`, signed with the contents of its
    /// [`Self::signed_by_file`], `key`. Its packages are pinned by the host
    /// of its URI, which also matches other archives on that host.
    pub fn archive(&self, name: &str, key: Option<Vec<u8>>) -> anyhow::Result<ExtraArchive> {
        if self.signed_by_file().is_some() != key.is_some() {
            bail!("{name} needs the key of its signed-by file, and only then");
        }
        let options = self
            .options
            .iter()
            .filter(|(k, _)| key.is_none() || k != "signed-by")
            .map(|(k, value)| (deb822_field(k), value.replace(',', " ")))
            .collect();
        let host = self.uri.split_once("://").map_or("", |(_, rest)| rest);
        let host = host.split('/').next().unwrap_or_default();
        let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
        // apt's origin is the bare host
        let host = match host.rsplit_once(':') {
            Some((host, port)) if port.bytes().all(|b| b.is_ascii_digit()) => host,
            _ => host,
        };
        Ok(ExtraArchive {
            name: name.to_string(),
            types: vec![self.kind.clone()],
            uri: self.uri.clone(),
            suite: self.suite.clone(),
            components: self.components.clone(),
            options,
            key,
            pin: format!("origin \"{host}\""),
        })
    }
}

/// The deb822 field of a one-line sources option: the same name, but
/// `arch`, `lang` and `target` are spelled out, and `+`/`-` add to or
/// remove from a default.
fn deb822_field(option: &str) -> String {
    let (base, suffix) = match (option.strip_suffix('+'), option.strip_suffix('-')) {
        (Some(base), _) => (base, "-Add"),
        (_, Some(base)) => (base, "-Remove"),
        _ => (option, ""),
    };
    let base = match base {
        "arch" => "Architectures".to_string(),
        "lang" => "Languages".to_string(),
        "target" => "Targets".to_string(),
        other => other
            .split('-')
            .map(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join("-"),
    };
    base + suffix
}

/// A Launchpad PPA, `ppa:<owner>/<name>[@<suite>]`: the suite is the
/// Ubuntu release to take its packages for, by default the target's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ppa {
    pub owner: String,
    pub name: String,
    pub suite: Option<String>,
}

impl FromStr for Ppa {
    type Err = String;

    fn from_str(spec: &str) -> Result<Self, String> {
        let invalid = || format!("'{spec}' is no ppa:<owner>/<name>[@<suite>]");
        let rest = spec.strip_prefix("ppa:").ok_or_else(invalid)?;
        let (path, suite) = match rest.split_once('@') {
            Some((path, suite)) => (path, Some(suite)),
            None => (rest, None),
        };
        let valid = |part: &str| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
        };
        match path.split_once('/') {
            Some((owner, name)) if valid(owner) && valid(name) && suite.is_none_or(valid) => {
                Ok(Self {
                    owner: owner.to_string(),
                    name: name.to_string(),
                    suite: suite.map(str::to_string),
                })
            }
            _ => Err(invalid()),
        }
    }
}

impl Ppa {
    /// Its Launchpad API resource, which names its signing key.
    pub fn api_url(&self) -> String {
        format!(
            "https://api.launchpad.net/devel/~{}/+archive/ubuntu/{}",
            self.owner, self.name
        )
    }

    /// It as the archive `name` of a build for `distro`, signed with the
    /// ASCII-armored `key`.
    pub fn archive(
        &self,
        name: &str,
        distro: &DistroVersion,
        key: String,
    ) -> anyhow::Result<ExtraArchive> {
        let (owner, ppa) = (&self.owner, &self.name);
        let suite = match (&self.suite, &distro.distro) {
            (Some(suite), _) => suite.clone(),
            (None, Distro::Ubuntu) => distro.codename.clone(),
            (None, other) => bail!(
                "PPAs are built for Ubuntu releases: pick one for {other} {} with \
                 ppa:{owner}/{ppa}@<suite>",
                distro.codename
            ),
        };
        // Launchpad leaves the name out of the Origin of a PPA named `ppa`
        let origin = match ppa.as_str() {
            "ppa" => format!("LP-PPA-{owner}"),
            _ => format!("LP-PPA-{owner}-{ppa}"),
        };
        Ok(ExtraArchive {
            name: name.to_string(),
            types: vec!["deb".to_string(), "deb-src".to_string()],
            uri: format!("https://ppa.launchpadcontent.net/{owner}/{ppa}/ubuntu"),
            suite,
            components: vec!["main".to_string()],
            options: Vec::new(),
            key: Some(key.into_bytes()),
            pin: format!("release o={origin}"),
        })
    }
}

/// An archive beyond the distro's, with what apt needs to use it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtraArchive {
    /// What `--package <package>/<name>` calls it.
    pub name: String,
    /// `deb`, `deb-src` or both.
    pub types: Vec<String>,
    pub uri: String,
    pub suite: String,
    pub components: Vec<String>,
    /// More fields of its deb822 sources paragraph.
    pub options: Vec<(String, String)>,
    /// The key its Release file is signed with, ASCII-armored or binary;
    /// without it, apt's trusted keys or its options decide.
    pub key: Option<Vec<u8>>,
    /// The apt preferences `Pin` of its packages.
    pub pin: String,
}

impl ExtraArchive {
    /// Where its key goes, relative to the environment's root: apt tells
    /// armored from binary keys by the extension.
    pub fn key_path(&self) -> Option<String> {
        let key = self.key.as_ref()?;
        let name: String = self
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let extension = match key.starts_with(b"-----BEGIN PGP") {
            true => "asc",
            false => "gpg",
        };
        Some(format!("etc/apt/keyrings/debmagic-{name}.{extension}"))
    }

    /// Its deb822 sources paragraph.
    fn sources(&self) -> String {
        let mut fields = vec![
            ("Types".to_string(), self.types.join(" ")),
            ("URIs".to_string(), self.uri.clone()),
            ("Suites".to_string(), self.suite.clone()),
        ];
        if !self.components.is_empty() {
            fields.push(("Components".to_string(), self.components.join(" ")));
        }
        fields.extend(self.options.iter().cloned());
        fields.extend(
            self.key_path()
                .map(|path| ("Signed-By".to_string(), format!("/{path}"))),
        );
        fields
            .iter()
            .map(|(field, value)| format!("{field}: {value}\n"))
            .collect()
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
    /// Taken all newer packages from, unless some are picked from them.
    pub extra: Vec<ExtraArchive>,
}

impl Archives {
    /// The archives for a build for `distro`: all of experimental for the
    /// experimental release, which only adds to unstable.
    pub fn for_distro(
        distro: &DistroVersion,
        proposed: bool,
        experimental: bool,
        packages: Packages,
        extra: Vec<ExtraArchive>,
    ) -> anyhow::Result<Self> {
        let archives = Self {
            proposed,
            experimental: experimental || is_experimental(distro),
            packages,
            extra,
        };
        let mut key_paths = Vec::new();
        for archive in &archives.extra {
            if ARCHIVES.contains(&archive.name.as_str()) {
                bail!("'{}' names a built-in archive", archive.name);
            }
            let Some(key_path) = archive.key_path() else {
                continue;
            };
            if key_paths.contains(&key_path) {
                bail!(
                    "archive names clash in {key_path}, rename '{}'",
                    archive.name
                );
            }
            key_paths.push(key_path);
        }
        let known: Vec<&str> = ARCHIVES
            .iter()
            .copied()
            .chain(archives.extra.iter().map(|archive| archive.name.as_str()))
            .collect();
        let release = format!("{} {}", distro.distro, distro.codename);
        for (pattern, settings) in &archives.packages {
            match settings.from.as_deref() {
                Some(from) if !known.contains(&from) => bail!(
                    "unknown archive '{from}' for {pattern}, known are {}",
                    known.join(", ")
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
        self.uses(PROPOSED) || self.uses(EXPERIMENTAL) || !self.extra.is_empty()
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

    let components = components.join(" ");
    let mut groups = vec![(
        mirror.unwrap_or(archive),
        suites.join(" "),
        components.clone(),
    )];
    if debian && release.contains(&security) {
        groups.push((DEBIAN_SECURITY_ARCHIVE, security, components));
    }
    let mut paragraphs: Vec<String> = groups
        .into_iter()
        .map(|(uri, suites, components)| {
            format!("Types: deb deb-src\nURIs: {uri}\nSuites: {suites}\nComponents: {components}\n")
        })
        .collect();
    paragraphs.extend(archives.extra.iter().map(ExtraArchive::sources));
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
/// - with only some packages from it, the rest of a proposed suite or an
///   extra archive only upgrades installed packages, also on Debian, which
///   doesn't mark them NotAutomatic.
pub fn preferences_file(distro: &DistroVersion, archives: &Archives) -> Option<String> {
    let mut pins = Vec::new();
    for (pattern, settings) in &archives.packages {
        match (&settings.version, &settings.from) {
            (Some(version), _) => pins.push(pin(pattern, &format!("version {version}"), 1001)),
            (None, Some(from)) => {
                for release in archive_pins(distro, archives, from) {
                    pins.push(pin(pattern, &release, 990));
                }
            }
            (None, None) => {}
        }
    }
    for extra in archives
        .extra
        .iter()
        .filter(|extra| archives.uses(&extra.name))
    {
        pins.push(pin("*", &extra.pin, 100));
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

/// The apt preferences pins of `archive`'s packages.
fn archive_pins(distro: &DistroVersion, archives: &Archives, archive: &str) -> Vec<String> {
    let suites = match archive {
        PROPOSED => proposed_suite(distro).into_iter().collect(),
        EXPERIMENTAL => vec![EXPERIMENTAL.to_string()],
        RELEASE => release_suites(distro),
        _ => {
            let extra = archives.extra.iter().filter(|extra| extra.name == archive);
            return extra.map(|extra| extra.pin.clone()).collect();
        }
    };
    suites
        .iter()
        .map(|suite| release_pin(distro, suite))
        .collect()
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
        Archives::for_distro(distro, proposed, false, packages.collect(), Vec::new()).unwrap()
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
        assert!(Archives::for_distro(&distro(name), false, false, packages, Vec::new()).is_err());
    }

    #[test]
    fn experimental_only_adds_to_debian_unstable_and_testing() {
        assert!(
            Archives::for_distro(&distro("noble"), false, true, Packages::new(), Vec::new())
                .is_err()
        );
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
        let extra =
            Archives::for_distro(&unstable, false, true, Packages::new(), Vec::new()).unwrap();
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

    #[test_case("ppa:me/stuff", "me", "stuff", None ; "plain")]
    #[test_case("ppa:me/stuff@noble", "me", "stuff", Some("noble") ; "suite")]
    fn ppa_spec(spec: &str, owner: &str, name: &str, suite: Option<&str>) {
        let ppa = spec.parse::<Ppa>().unwrap();
        assert_eq!((ppa.owner.as_str(), ppa.name.as_str()), (owner, name));
        assert_eq!(ppa.suite.as_deref(), suite);
    }

    #[test_case("me/stuff" ; "no prefix")]
    #[test_case("ppa:me" ; "no name")]
    #[test_case("ppa:me/stuff@" ; "empty suite")]
    #[test_case("ppa:me/st uff" ; "whitespace")]
    fn ppa_spec_rejects(spec: &str) {
        assert!(spec.parse::<Ppa>().is_err());
    }

    fn ppa(spec: &str, distro: &DistroVersion) -> anyhow::Result<ExtraArchive> {
        spec.parse::<Ppa>()
            .unwrap()
            .archive(spec, distro, "-----BEGIN PGP".to_string())
    }

    #[test]
    fn ppa_named_ppa_has_the_owner_as_origin() {
        let noble = distro("noble");
        assert_eq!(
            ppa("ppa:me/ppa", &noble).unwrap().pin,
            "release o=LP-PPA-me"
        );
        assert_eq!(
            ppa("ppa:me/stuff", &noble).unwrap().pin,
            "release o=LP-PPA-me-stuff"
        );
    }

    #[test]
    fn ppa_for_debian_needs_a_suite() {
        let trixie = distro("trixie");
        assert!(ppa("ppa:me/stuff", &trixie).is_err());
        assert_eq!(ppa("ppa:me/stuff@noble", &trixie).unwrap().suite, "noble");
    }

    #[test]
    fn ppa_is_signed_by_its_key_and_pinned_when_picked_from() {
        let noble = distro("noble");
        let extra = vec![ppa("ppa:me/stuff", &noble).unwrap()];
        let packages = [parse_package("libfoo1/ppa:me/stuff").unwrap()].into();
        let archives = Archives::for_distro(&noble, false, false, packages, extra).unwrap();
        let sources = sources_file(&noble, None, &archives, "amd64").unwrap();
        assert!(
            sources.ends_with(
                "\n\nTypes: deb deb-src\nURIs: https://ppa.launchpadcontent.net/me/stuff/ubuntu\n\
                 Suites: noble\nComponents: main\n\
                 Signed-By: /etc/apt/keyrings/debmagic-ppa-me-stuff.asc\n"
            ),
            "{sources}"
        );
        assert_eq!(
            preferences_file(&noble, &archives).unwrap(),
            "Package: libfoo1\nPin: release o=LP-PPA-me-stuff\nPin-Priority: 990\n\n\
             Package: *\nPin: release o=LP-PPA-me-stuff\nPin-Priority: 100\n"
        );

        let all = Archives::for_distro(&noble, false, false, Packages::new(), archives.extra);
        let all = all.unwrap();
        assert!(all.adds_suites());
        assert_eq!(preferences_file(&noble, &all), None);
    }

    #[test]
    fn extra_archives_must_not_shadow_or_clash() {
        let noble = distro("noble");
        let mut proposed = ppa("ppa:me/stuff", &noble).unwrap();
        proposed.name = "proposed".to_string();
        let mut clash = ppa("ppa:me/stuff", &noble).unwrap();
        clash.name = "ppa:me-stuff".to_string();
        let extra = |extra| Archives::for_distro(&noble, false, false, Packages::new(), extra);
        assert!(extra(vec![proposed]).is_err());
        assert!(extra(vec![ppa("ppa:me/stuff", &noble).unwrap(), clash]).is_err());
    }

    #[test]
    fn deb_line_options_become_fields_and_its_key_file_is_copied() {
        let line: DebLine =
            "deb [ arch=amd64,i386 signed-by=/usr/share/keyrings/x.gpg Trusted=yes ] \
                             https://me@example.org:8080/debian bookworm main contrib"
                .parse()
                .unwrap();
        assert_eq!(line.signed_by_file(), Some("/usr/share/keyrings/x.gpg"));
        assert!(line.archive("mine", None).is_err());
        let archive = line.archive("mine", Some(b"\x99binary".to_vec())).unwrap();
        assert_eq!(
            archive.sources(),
            "Types: deb\nURIs: https://me@example.org:8080/debian\nSuites: bookworm\n\
             Components: main contrib\nArchitectures: amd64 i386\nTrusted: yes\n\
             Signed-By: /etc/apt/keyrings/debmagic-mine.gpg\n"
        );
        assert_eq!(archive.pin, "origin \"example.org\"");
    }

    #[test]
    fn deb_line_without_key_file() {
        let flat = "deb [trusted=yes] file:/srv/repo ./"
            .parse::<DebLine>()
            .unwrap();
        let archive = flat.archive("local", None).unwrap();
        assert_eq!(archive.key_path(), None);
        assert_eq!(
            archive.sources(),
            "Types: deb\nURIs: file:/srv/repo\nSuites: ./\nTrusted: yes\n"
        );
        assert_eq!(archive.pin, "origin \"\"");

        let by_fingerprint = "deb-src [signed-by=0123ABCD] http://x/ s main";
        let line = by_fingerprint.parse::<DebLine>().unwrap();
        assert_eq!(line.signed_by_file(), None);
        assert!(
            line.archive("x", None)
                .unwrap()
                .sources()
                .ends_with("Signed-By: 0123ABCD\n")
        );
    }

    #[test_case("rpm http://x s main" ; "no deb")]
    #[test_case("deb http://x" ; "no suite")]
    #[test_case("deb http://x s" ; "suite without components")]
    #[test_case("deb http://x ./ main" ; "path with components")]
    #[test_case("deb [arch=amd64 http://x s main" ; "unterminated options")]
    #[test_case("deb [arch] http://x s main" ; "option without value")]
    #[test_case("deb [signed-by=/a.gpg,/b.gpg] http://x s main" ; "two key files")]
    fn deb_line_rejects(spec: &str) {
        assert!(spec.parse::<DebLine>().is_err());
    }

    #[test_case("arch", "Architectures")]
    #[test_case("arch+", "Architectures-Add")]
    #[test_case("target-", "Targets-Remove")]
    #[test_case("by-hash", "By-Hash")]
    fn deb822_fields(option: &str, field: &str) {
        assert_eq!(deb822_field(option), field);
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
