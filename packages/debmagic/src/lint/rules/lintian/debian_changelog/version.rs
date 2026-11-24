//! Lintian's `Lintian::Changelog::Version` and dpkg `verrevcmp`, kept off
//! [`DebianChangelog`](crate::lint::debian_changelog::DebianChangelog).

use std::sync::LazyLock;

use debmagic_common::debian::version::verrevcmp;
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
    /// The Debian revision (the native version itself for a native package),
    /// without a source NMU suffix.
    pub maintainer_revision: String,
    /// `.N` (`+nmuN` for a native package) of a source NMU, without the
    /// separator; empty when the version is not an NMU.
    pub source_nmu: String,
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
        maintainer_revision,
        source_nmu,
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

/// `versions_gt`: dpkg upstream comparison (the whole string through
/// [`verrevcmp`]). Equal strings are not greater.
pub(super) fn versions_gt(left: &str, right: &str) -> bool {
    if left == right {
        return false;
    }
    verrevcmp(left, right).is_gt()
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
    fn revision_and_source_nmu_are_split_off() {
        let nmu = parse_version("1.0-1.2", false).expect("parses");
        assert_eq!(nmu.maintainer_revision, "1");
        assert_eq!(nmu.source_nmu, "2");
        let plain = parse_version("1.0-1~exp1", false).expect("parses");
        assert_eq!(plain.maintainer_revision, "1~exp1");
        assert_eq!(plain.source_nmu, "");
        let native = parse_version("2.5+nmu1", true).expect("parses");
        assert_eq!(native.maintainer_revision, "2.5");
        assert_eq!(native.source_nmu, "1");
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
    }
}
