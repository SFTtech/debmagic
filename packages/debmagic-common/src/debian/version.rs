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
/// with dpkg semantics: alternating non-digit / digit chunks, non-digits
/// compare lexically with `~` sorting before everything including the end.
fn compare_version_parts(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a = a;
    let mut b = b;
    loop {
        let a_empty = a.is_empty();
        let b_empty = b.is_empty();
        if a_empty && b_empty {
            return std::cmp::Ordering::Equal;
        }
        // `~` sorts before everything, including the empty remainder
        let a_tilde = a.starts_with('~');
        let b_tilde = b.starts_with('~');
        match (a_tilde, b_tilde) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            (true, true) => {
                a = &a[1..];
                b = &b[1..];
                continue;
            }
            (false, false) => {}
        }
        if a_empty {
            return std::cmp::Ordering::Less;
        }
        if b_empty {
            return std::cmp::Ordering::Greater;
        }
        // non-digit prefix: compare lexically until digits start on either side
        let a_nondigits = non_digit_prefix_len(a);
        let b_nondigits = non_digit_prefix_len(b);
        if a_nondigits > 0 || b_nondigits > 0 {
            let a_prefix = &a[..a_nondigits];
            let b_prefix = &b[..b_nondigits];
            match a_prefix.cmp(b_prefix) {
                std::cmp::Ordering::Equal => {
                    a = &a[a_nondigits..];
                    b = &b[b_nondigits..];
                }
                other => return other,
            }
        } else {
            // both start with digits: compare numerically
            let a_digits = digit_prefix_len(a);
            let b_digits = digit_prefix_len(b);
            let a_num: u64 = a[..a_digits].parse().unwrap_or(u64::MAX);
            let b_num: u64 = b[..b_digits].parse().unwrap_or(u64::MAX);
            match a_num.cmp(&b_num) {
                std::cmp::Ordering::Equal => {
                    a = &a[a_digits..];
                    b = &b[b_digits..];
                }
                other => return other,
            }
        }
    }
}

fn non_digit_prefix_len(s: &str) -> usize {
    s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len())
}

fn digit_prefix_len(s: &str) -> usize {
    s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len())
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
    fn test_version_compare(a: &str, b: &str, expected: std::cmp::Ordering) {
        let a = PackageVersion::from_str(a).unwrap();
        let b = PackageVersion::from_str(b).unwrap();
        assert_eq!(a.cmp(&b), expected);
    }
}
