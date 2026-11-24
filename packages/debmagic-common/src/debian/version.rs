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

/// libapt-pkg's `debVersioningSystem::CmpVersion`, which lintian compares
/// versions with (`Lintian::Relation::Version` through `AptPkg`). Unlike
/// [`PackageVersion`] it never rejects a version: the epoch is everything
/// before the first `:` (an all-zero epoch is no epoch), the revision
/// everything after the last `-`, and an empty revision after a `-` sorts
/// below a missing one.
pub fn apt_compare(left: &str, right: &str) -> std::cmp::Ordering {
    apt_cmp_version(left.as_bytes(), right.as_bytes()).cmp(&0)
}

/// The byte at `index`, or 0 outside the text: libapt reads C strings and
/// peeks one byte past a fragment's end, where the next delimiter or the
/// terminating NUL sits.
fn byte_at(text: &[u8], index: isize) -> u8 {
    usize::try_from(index)
        .ok()
        .and_then(|index| text.get(index))
        .copied()
        .unwrap_or(0)
}

/// libapt's `order`: `char` is signed, so a byte above 0x7f weighs
/// `(byte as i8) + 256`.
fn apt_order(byte: u8) -> i32 {
    if byte.is_ascii_digit() || byte == 0 {
        0
    } else if byte.is_ascii_alphabetic() {
        i32::from(byte)
    } else if byte == b'~' {
        -1
    } else {
        i32::from(byte as i8) + 256
    }
}

/// libapt's `CmpFragment` on `a[a_start..a_end]` and `b[b_start..b_end]`.
fn apt_cmp_fragment(
    a: &[u8],
    a_start: isize,
    a_end: isize,
    b: &[u8],
    b_start: isize,
    b_end: isize,
) -> i32 {
    let (mut lhs, mut rhs) = (a_start, b_start);
    while lhs != a_end && rhs != b_end {
        let mut first_diff = 0;
        while lhs != a_end
            && rhs != b_end
            && (!byte_at(a, lhs).is_ascii_digit() || !byte_at(b, rhs).is_ascii_digit())
        {
            let (vc, rc) = (apt_order(byte_at(a, lhs)), apt_order(byte_at(b, rhs)));
            if vc != rc {
                return vc - rc;
            }
            lhs += 1;
            rhs += 1;
        }
        while byte_at(a, lhs) == b'0' {
            lhs += 1;
        }
        while byte_at(b, rhs) == b'0' {
            rhs += 1;
        }
        while byte_at(a, lhs).is_ascii_digit() && byte_at(b, rhs).is_ascii_digit() {
            if first_diff == 0 {
                first_diff = i32::from(byte_at(a, lhs)) - i32::from(byte_at(b, rhs));
            }
            lhs += 1;
            rhs += 1;
        }
        if byte_at(a, lhs).is_ascii_digit() {
            return 1;
        }
        if byte_at(b, rhs).is_ascii_digit() {
            return -1;
        }
        if first_diff != 0 {
            return first_diff;
        }
    }
    if lhs == a_end && rhs == b_end {
        return 0;
    }
    if lhs == a_end {
        return if byte_at(b, rhs) == b'~' { 1 } else { -1 };
    }
    if rhs == b_end {
        return if byte_at(a, lhs) == b'~' { -1 } else { 1 };
    }
    1
}

/// libapt's `DoCmpVersion`, with its pointer arithmetic as signed indices.
fn apt_cmp_version(a: &[u8], b: &[u8]) -> i32 {
    let len = |text: &[u8]| isize::try_from(text.len()).unwrap_or(isize::MAX);
    let (a_end, b_end) = (len(a), len(b));
    let colon = |text: &[u8]| {
        text.iter()
            .position(|&byte| byte == b':')
            .and_then(|index| isize::try_from(index).ok())
    };
    let (mut a_start, mut b_start) = (0, 0);
    let mut lhs = colon(a).unwrap_or(0);
    let mut rhs = colon(b).unwrap_or(0);

    // A zero epoch is the same as no epoch.
    if lhs != a_start {
        while byte_at(a, a_start) == b'0' {
            a_start += 1;
        }
        if a_start == lhs {
            a_start += 1;
            lhs += 1;
        }
    }
    if rhs != b_start {
        while byte_at(b, b_start) == b'0' {
            b_start += 1;
        }
        if b_start == rhs {
            b_start += 1;
            rhs += 1;
        }
    }

    let result = apt_cmp_fragment(a, a_start, lhs, b, b_start, rhs);
    if result != 0 {
        return result;
    }

    // Skip the `:`.
    if lhs != a_start {
        lhs += 1;
    }
    if rhs != b_start {
        rhs += 1;
    }

    let last_dash = |text: &[u8], from: isize, end: isize| {
        let from_index = usize::try_from(from).unwrap_or(0).min(text.len());
        text[from_index..]
            .iter()
            .rposition(|&byte| byte == b'-')
            .and_then(|index| isize::try_from(from_index + index).ok())
            .unwrap_or(end)
    };
    let mut dlhs = last_dash(a, lhs, a_end);
    let mut drhs = last_dash(b, rhs, b_end);

    let result = apt_cmp_fragment(a, lhs, dlhs, b, rhs, drhs);
    if result != 0 {
        return result;
    }

    // Skip the `-`.
    if dlhs != lhs {
        dlhs += 1;
    }
    if drhs != rhs {
        drhs += 1;
    }

    // No revision after a `-` compares like `-0`.
    let zero: &[u8] = b"0";
    match (byte_at(a, dlhs - 1) == b'-', byte_at(b, drhs - 1) == b'-') {
        (true, true) => apt_cmp_fragment(a, dlhs, a_end, b, drhs, b_end),
        (true, false) => apt_cmp_fragment(a, dlhs, a_end, zero, 0, 1),
        (false, true) => apt_cmp_fragment(zero, 0, 1, b, drhs, b_end),
        (false, false) => 0,
    }
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

    /// Pairs and the sign libapt-pkg's `CmpVersion` gives them, recorded
    /// with Perl `AptPkg` (the module lintian uses); see
    /// `testdata/apt_version_compare.tsv`.
    #[test]
    fn apt_compare_matches_libapt() {
        let data = include_str!("testdata/apt_version_compare.tsv");
        let mut checked = 0;
        for line in data.lines() {
            let mut fields = line.split('\t');
            let (Some(a), Some(b), Some(expected)) = (fields.next(), fields.next(), fields.next())
            else {
                panic!("malformed line {line:?}");
            };
            let expected: i32 = expected.parse().expect("sign");
            assert_eq!(apt_compare(a, b), expected.cmp(&0), "{a:?} vs {b:?}");
            checked += 1;
        }
        assert!(checked > 1000);
    }
}
