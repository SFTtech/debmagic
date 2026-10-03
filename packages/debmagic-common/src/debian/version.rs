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
            .then_with(|| compare_version_parts(&self.upstream, &other.upstream))
            .then_with(|| {
                compare_version_parts(
                    self.revision.as_deref().unwrap_or(""),
                    other.revision.as_deref().unwrap_or(""),
                )
            })
    }
}

/// Compare one non-epoch part of a debian version (upstream or revision)
/// like dpkg's `verrevcmp`: alternating non-digit / digit runs; non-digit
/// runs compare character-wise by [`char_order`], digit runs numerically.
fn compare_version_parts(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a = a.as_bytes();
    let mut b = b.as_bytes();
    while !a.is_empty() || !b.is_empty() {
        while a.first().is_some_and(|c| !c.is_ascii_digit())
            || b.first().is_some_and(|c| !c.is_ascii_digit())
        {
            let (a_order, b_order) = (char_order(a.first()), char_order(b.first()));
            if a_order != b_order {
                return a_order.cmp(&b_order);
            }
            // equal orders are never 0 here, so both sides have a non-digit
            a = &a[1..];
            b = &b[1..];
        }

        let (a_digits, a_rest) = a.split_at(a.iter().take_while(|c| c.is_ascii_digit()).count());
        let (b_digits, b_rest) = b.split_at(b.iter().take_while(|c| c.is_ascii_digit()).count());
        let trim = |digits: &[u8]| -> usize { digits.iter().take_while(|&&c| c == b'0').count() };
        let (a_digits, b_digits) = (&a_digits[trim(a_digits)..], &b_digits[trim(b_digits)..]);
        // arbitrary length, so no integer parsing: longer means larger
        match a_digits
            .len()
            .cmp(&b_digits.len())
            .then(a_digits.cmp(b_digits))
        {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
        a = a_rest;
        b = b_rest;
    }
    std::cmp::Ordering::Equal
}

/// dpkg's sort weight of one character in a non-digit run: `~` sorts before
/// the end of the part, letters before every other character.
fn char_order(c: Option<&u8>) -> i32 {
    match c {
        Some(b'~') => -1,
        None => 0,
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
}
