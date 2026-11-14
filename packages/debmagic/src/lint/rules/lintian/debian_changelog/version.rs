//! Lintian's `Lintian::Changelog::Version` and dpkg `verrevcmp`, kept off
//! [`DebianChangelog`](crate::lint::debian_changelog::DebianChangelog).

use std::sync::LazyLock;

use regex::Regex;

static NON_NATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:([0-9]+):)?([A-Za-z0-9.+~-]+?)(?:-([A-Za-z0-9.+~]+?)(?:\.([A-Za-z0-9.+~]+))?)?(?:\+b([0-9]+))?$",
    )
    .expect("valid regex")
});

static NATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:([0-9]+):)?()([A-Za-z0-9.+~]+?)(?:\+nmu([A-Za-z0-9.+~]+))?(?:\+b([0-9]+))?$")
        .expect("valid regex")
});

static HISTORICAL_NO_EPOCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[^:]+:)?([^:]+)$").expect("valid regex"));

/// A literal that `assign` accepts for this nativeness. Reconstruction must
/// equal the literal, so a non-native version without a Debian revision
/// (`6.1`, `11`) is rejected: the rebuild always writes `upstream-`.
pub(super) struct ParsedVersion {
    pub literal: String,
    pub epoch: String,
    pub no_epoch: String,
    pub upstream: String,
}

pub(super) fn parse_version(literal: &str, native: bool) -> Option<ParsedVersion> {
    let caps = if native {
        NATIVE.captures(literal)?
    } else {
        NON_NATIVE.captures(literal)?
    };
    let group = |index: usize| {
        caps.get(index)
            .map(|found| found.as_str())
            .unwrap_or("")
            .to_string()
    };
    let epoch = group(1);
    let upstream = group(2);
    let maintainer_revision = group(3);
    let source_nmu = group(4);
    let binary_nmu = group(5);

    let source_nmu_string = if source_nmu.is_empty() {
        String::new()
    } else if native {
        format!("+nmu{source_nmu}")
    } else {
        format!(".{source_nmu}")
    };
    let debian_source = format!("{maintainer_revision}{source_nmu_string}");
    let binary_string = if binary_nmu.is_empty() {
        String::new()
    } else {
        format!("+b{binary_nmu}")
    };
    let debian_no_epoch = format!("{debian_source}{binary_string}");
    let upstream_string = if upstream.is_empty() {
        String::new()
    } else {
        format!("{upstream}-")
    };
    let no_epoch = format!("{upstream_string}{debian_no_epoch}");
    let epoch_string = if epoch.is_empty() {
        String::new()
    } else {
        format!("{epoch}:")
    };
    let reconstructed = format!("{epoch_string}{no_epoch}");
    if reconstructed != literal {
        return None;
    }
    Some(ParsedVersion {
        literal: literal.to_string(),
        epoch,
        no_epoch,
        upstream,
    })
}

/// `^(?:[^:]+:)?([^:]+)$` — the historical version with one leading epoch
/// removed. Not the parsed `no_epoch` (nativeness may differ).
pub(super) fn historical_no_epoch(version: &str) -> Option<&str> {
    HISTORICAL_NO_EPOCH
        .captures(version)
        .and_then(|caps| caps.get(1))
        .map(|found| found.as_str())
}

/// `versions_gt`: dpkg upstream comparison. Equal strings are not greater.
pub(super) fn versions_gt(left: &str, right: &str) -> bool {
    if left == right {
        return false;
    }
    verrevcmp(left.as_bytes(), right.as_bytes()) > 0
}

fn verrevcmp(mut left: &[u8], mut right: &[u8]) -> i32 {
    while !left.is_empty() || !right.is_empty() {
        let mut first_diff = 0i32;
        while left.first().is_some_and(|byte| !byte.is_ascii_digit())
            || right.first().is_some_and(|byte| !byte.is_ascii_digit())
        {
            let left_order = order(left.first().copied());
            let right_order = order(right.first().copied());
            if left_order != right_order {
                return left_order - right_order;
            }
            if !left.is_empty() {
                left = &left[1..];
            }
            if !right.is_empty() {
                right = &right[1..];
            }
        }
        while left.first() == Some(&b'0') {
            left = &left[1..];
        }
        while right.first() == Some(&b'0') {
            right = &right[1..];
        }
        while left.first().is_some_and(|byte| byte.is_ascii_digit())
            && right.first().is_some_and(|byte| byte.is_ascii_digit())
        {
            if first_diff == 0 {
                first_diff = i32::from(left[0]) - i32::from(right[0]);
            }
            left = &left[1..];
            right = &right[1..];
        }
        if left.first().is_some_and(|byte| byte.is_ascii_digit()) {
            return 1;
        }
        if right.first().is_some_and(|byte| byte.is_ascii_digit()) {
            return -1;
        }
        if first_diff != 0 {
            return first_diff;
        }
    }
    0
}

fn order(byte: Option<u8>) -> i32 {
    match byte {
        None => 0,
        Some(byte) if byte.is_ascii_digit() => 0,
        Some(byte) if byte.is_ascii_alphabetic() => i32::from(byte),
        Some(b'~') => -1,
        Some(0) => 0,
        Some(byte) => i32::from(byte) + 256,
    }
}

/// `s/^\d+://` — one leading epoch.
pub(super) fn strip_epoch(version: &str) -> &str {
    let Some(index) = version.find(':') else {
        return version;
    };
    if index > 0 && version[..index].bytes().all(|byte| byte.is_ascii_digit()) {
        &version[index + 1..]
    } else {
        version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_native_splits_revision_and_keeps_no_epoch() {
        let parsed = parse_version("1:1.0-1", false).expect("parses");
        assert_eq!(parsed.epoch, "1");
        assert_eq!(parsed.upstream, "1.0");
        assert_eq!(parsed.no_epoch, "1.0-1");
        assert_eq!(parsed.literal, "1:1.0-1");
    }

    #[test]
    fn non_native_without_revision_is_rejected() {
        assert!(parse_version("6.1", false).is_none());
        assert!(parse_version("11", false).is_none());
    }

    #[test]
    fn special_characters_keep_the_hyphen_in_upstream() {
        let parsed = parse_version("2:1.5+important~personal1-stable-7", false).expect("parses");
        assert_eq!(parsed.epoch, "2");
        assert_eq!(parsed.upstream, "1.5+important~personal1-stable");
    }

    #[test]
    fn native_revision_has_no_upstream() {
        let parsed = parse_version("2:1.0", true).expect("parses");
        assert_eq!(parsed.epoch, "2");
        assert_eq!(parsed.upstream, "");
        assert_eq!(parsed.no_epoch, "1.0");
        assert!(parse_version("0.1-2", true).is_none());
        let introduced = parse_version("1:99999999", true).expect("parses");
        assert_eq!(introduced.epoch, "1");
        assert_eq!(introduced.upstream, "");
    }

    #[test]
    fn upstream_order_matches_dpkg() {
        assert!(versions_gt("99999999", "1.0"));
        assert!(!versions_gt("0.9", "1.0"));
        assert!(!versions_gt("1.0", "1.0"));
        assert!(!versions_gt("0.0.1", "1.5+important~personal1-stable"));
        assert!(versions_gt("1.0", "1.0~rc1"));
    }

    #[test]
    fn historical_strips_one_epoch() {
        assert_eq!(historical_no_epoch("1:1.0-1"), Some("1.0-1"));
        assert_eq!(historical_no_epoch("1.0-1"), Some("1.0-1"));
        assert_eq!(historical_no_epoch("2:1.0"), Some("1.0"));
        assert_eq!(strip_epoch("1:1.0-1"), "1.0-1");
    }
}
