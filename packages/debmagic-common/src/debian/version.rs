use std::fmt;
use std::str::FromStr;

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVersion {
    // distro packaging override base version (default is 0)
    epoch: Option<u32>,

    // upstream package version
    upstream: String,

    // packaging (linux distro) revision
    revision: Option<String>,
}

impl PackageVersion {
    pub fn new(epoch: Option<u32>, upstream: String, revision: Option<String>) -> Self {
        Self {
            epoch,
            upstream,
            revision,
        }
    }

    pub fn version(&self) -> String {
        let mut ret: String = "".to_owned();
        if let Some(epoch) = self.epoch
            && epoch != 0
        {
            ret.push_str(&format!("{}:", epoch));
        }
        ret.push_str(&self.upstream);
        if let Some(revision) = &self.revision {
            ret.push_str(&format!("-{}", revision));
        }
        ret
    }

    /// distro epoch plus upstream version
    pub fn epoch_upstream(&self) -> String {
        if let Some(epoch) = self.epoch
            && epoch != 0
        {
            return format!("{}:{}", epoch, self.upstream);
        }
        self.upstream.clone()
    }

    pub fn upstream_version(&self) -> &str {
        &self.upstream
    }

    /// upstream version plus packaging revision
    pub fn upstream_revision(&self) -> String {
        if let Some(revision) = &self.revision {
            return format!("{}-{}", self.upstream, revision);
        }
        self.upstream.clone()
    }
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.version())
    }
}

impl PartialOrd for PackageVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PackageVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let epoch = self.epoch.unwrap_or(0);
        let other_epoch = other.epoch.unwrap_or(0);
        epoch
            .cmp(&other_epoch)
            .then_with(|| verrevcmp(&self.upstream, &other.upstream))
            .then_with(|| {
                verrevcmp(
                    self.revision.as_deref().unwrap_or(""),
                    other.revision.as_deref().unwrap_or(""),
                )
            })
    }
}

/// dpkg's `verrevcmp` on one version part (upstream or revision), ported
/// literally from the C: alternating non-digit and digit runs, non-digit
/// runs compared character-wise by [`char_order`], digit runs numerically
/// at any length. lintian compares the same way through libapt-pkg.
pub fn verrevcmp(left: &str, right: &str) -> std::cmp::Ordering {
    let (mut left, mut right) = (left.as_bytes(), right.as_bytes());
    while !left.is_empty() || !right.is_empty() {
        let mut first_diff = 0i32;
        while left.first().is_some_and(|byte| !byte.is_ascii_digit())
            || right.first().is_some_and(|byte| !byte.is_ascii_digit())
        {
            let left_order = char_order(left.first());
            let right_order = char_order(right.first());
            if left_order != right_order {
                return left_order.cmp(&right_order);
            }
            left = left.get(1..).unwrap_or_default();
            right = right.get(1..).unwrap_or_default();
        }
        while left.first() == Some(&b'0') {
            left = &left[1..];
        }
        while right.first() == Some(&b'0') {
            right = &right[1..];
        }
        while left.first().is_some_and(u8::is_ascii_digit)
            && right.first().is_some_and(u8::is_ascii_digit)
        {
            if first_diff == 0 {
                first_diff = i32::from(left[0]) - i32::from(right[0]);
            }
            left = &left[1..];
            right = &right[1..];
        }
        if left.first().is_some_and(u8::is_ascii_digit) {
            return std::cmp::Ordering::Greater;
        }
        if right.first().is_some_and(u8::is_ascii_digit) {
            return std::cmp::Ordering::Less;
        }
        if first_diff != 0 {
            return first_diff.cmp(&0);
        }
    }
    std::cmp::Ordering::Equal
}

/// dpkg's sort weight of one character in a non-digit run: `~` sorts before
/// the end of the part, letters before every other character.
fn char_order(c: Option<&u8>) -> i32 {
    match c {
        Some(b'~') => -1,
        // dpkg and libapt-pkg read C strings: a NUL weighs like the end.
        None | Some(0) => 0,
        Some(c) if c.is_ascii_digit() => 0,
        Some(c) if c.is_ascii_alphabetic() => i32::from(*c),
        Some(c) => i32::from(*c) + 256,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct VersionParseError;

impl FromStr for PackageVersion {
    type Err = VersionParseError;

    fn from_str(version: &str) -> Result<Self, Self::Err> {
        let re_epoch_upstream = Regex::new(r"^(.*?)(-[^-]*)?$").map_err(|_| VersionParseError)?;
        let epoch_upstream = re_epoch_upstream.replace(version, "$1").to_string();

        // epoch = distro packaging override base version (default is 0)
        // pkg-info.mk uses the full version if no epoch is in it.
        // instead, we return "0" as oritinally intended if no epoch is in version.
        let epoch = if !version.contains(':') {
            Some(0)
        } else {
            let re_epoch = Regex::new(r"^([0-9]+):.*$").map_err(|_| VersionParseError)?;
            let epoch_str = re_epoch.replace(version, "$1").to_string();
            let parsed_epoch = epoch_str.parse::<u32>().map_err(|_| VersionParseError)?;
            Some(parsed_epoch)
        };

        let re_upstream = Regex::new(r"^([0-9]*:)?(.*?)$").map_err(|_| VersionParseError)?;
        let upstream = re_upstream.replace(&epoch_upstream, "$2").to_string();

        let re_revision = Regex::new(r"^.*?(-([^-]*))?$").map_err(|_| VersionParseError)?;
        let revision = re_revision.replace(version, "$2").to_string();

        // TODO: properly handle errors if we put in actual crap -> currently we return something nonsensical instead of returning an error

        Ok(Self {
            epoch,
            upstream,
            revision: if revision.is_empty() {
                None
            } else {
                Some(revision)
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    #[test_case(
        "1.2.3a.4-42.2-14ubuntu2~20.04.1", 
        &PackageVersion{epoch:Some(0), upstream:"1.2.3a.4-42.2".to_string(), revision:Some("14ubuntu2~20.04.1".to_string())})]
    #[test_case(
        "3:1.2.3a.4-42.2-14ubuntu2~20.04.1", 
        &PackageVersion{epoch:Some(3), upstream:"1.2.3a.4-42.2".to_string(), revision:Some("14ubuntu2~20.04.1".to_string())})]
    #[test_case(
        "3:1.2.3a.4ubuntu", 
        &PackageVersion{epoch:Some(3), upstream:"1.2.3a.4ubuntu".to_string(), revision:None})]
    #[test_case(
        "3:1.2.3a-4ubuntu", 
        &PackageVersion{epoch:Some(3), upstream:"1.2.3a".to_string(), revision:Some("4ubuntu".to_string())})]
    #[test_case(
        "3:1.2.3a-4ubuntu1", 
        &PackageVersion{epoch:Some(3), upstream:"1.2.3a".to_string(), revision:Some("4ubuntu1".to_string())})]
    fn test_version_parsing(version: &str, expected: &PackageVersion) {
        let parsed_version = PackageVersion::from_str(version).unwrap();

        // initial parsing works
        assert_eq!(&parsed_version, expected);
        // reverse formatting works as well
        assert_eq!(parsed_version.version(), version);
    }

    #[test_case("1.0", "1.0", std::cmp::Ordering::Equal; "equal")]
    #[test_case("1.1", "1.0", std::cmp::Ordering::Greater; "minor bump")]
    #[test_case("2.0", "10.0", std::cmp::Ordering::Less; "numeric not lexical")]
    #[test_case("1.0~rc1", "1.0", std::cmp::Ordering::Less; "prerelease before release")]
    #[test_case("1.0~rc1", "1.0~rc2", std::cmp::Ordering::Less; "prerelease order")]
    #[test_case("1.0-1", "1.0-1", std::cmp::Ordering::Equal; "revision equal")]
    #[test_case("1.0-2", "1.0-1", std::cmp::Ordering::Greater; "revision order")]
    #[test_case("1.0-1ubuntu1", "1.0-1", std::cmp::Ordering::Greater; "ubuntu after debian")]
    #[test_case("1.0+dfsg1", "1.0", std::cmp::Ordering::Greater; "dfsg suffix after")]
    #[test_case("1:0.9", "2.0", std::cmp::Ordering::Greater; "epoch wins")]
    #[test_case("1.2.3a.4-42.2-14ubuntu2", "1.2.3a.4-42.2-14ubuntu3", std::cmp::Ordering::Less; "complex")]
    #[test_case("2.0rc1", "2.0.1", std::cmp::Ordering::Less; "letters before non-letters")]
    #[test_case("1.0a", "1.0+", std::cmp::Ordering::Less; "letter before plus")]
    #[test_case("1.0a~b", "1.0a", std::cmp::Ordering::Less; "tilde inside a non-digit run")]
    #[test_case("1.0", "1.0a", std::cmp::Ordering::Less; "end before letter")]
    #[test_case("1.01", "1.1", std::cmp::Ordering::Equal; "leading zeros")]
    #[test_case("1.99999999999999999999", "1.100000000000000000000", std::cmp::Ordering::Less; "beyond u64")]
    fn test_version_compare(a: &str, b: &str, expected: std::cmp::Ordering) {
        let a = PackageVersion::from_str(a).unwrap();
        let b = PackageVersion::from_str(b).unwrap();
        assert_eq!(a.cmp(&b), expected);
    }

    #[test_case("1.0", "1.0", std::cmp::Ordering::Equal; "equal")]
    #[test_case("1.0~rc1", "1.0", std::cmp::Ordering::Less; "tilde before end")]
    #[test_case("0.0.1", "1.5+important~personal1-stable", std::cmp::Ordering::Less; "digits first")]
    #[test_case("1.0a\0", "1.0a", std::cmp::Ordering::Equal; "nul weighs like the end")]
    #[test_case("1.0\0a", "1.0a", std::cmp::Ordering::Less; "nul before letter")]
    #[test_case("", "~", std::cmp::Ordering::Greater; "empty after tilde")]
    fn test_verrevcmp(a: &str, b: &str, expected: std::cmp::Ordering) {
        assert_eq!(verrevcmp(a, b), expected);
        assert_eq!(verrevcmp(b, a), expected.reverse());
    }
}
