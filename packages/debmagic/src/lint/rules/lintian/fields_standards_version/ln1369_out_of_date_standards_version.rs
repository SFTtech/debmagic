use chrono::DateTime;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::{
    is_policy_release, is_three_part_number, latest_policy_version,
    latest_policy_version_normalized, policy_release_epoch, standards_version,
};

pub struct OutOfDateStandardsVersion;

declare_rule! {
    /// The Standards-Version is older than a normative Policy release that was
    /// already out when the latest changelog entry was written.
    ///
    /// Only a Standards-Version that is a known Policy release, with three
    /// numbers, is compared. Newer Policy releases (strictly after the declared
    /// one) are listed newest first, and only those whose version is three
    /// numbers, or three numbers ending in `.0`, count as normative. The Tag
    /// fires when one of them was released on or before the changelog's
    /// newest entry (the Policy release date stands in when there is no
    /// parseable date). Extra is `<version> (released <date>) (current is
    /// <latest>)`, with `<latest>` the newest Policy release's three numbers.
    /// Location is empty. Source tree and Source package. Info.
    OutOfDateStandardsVersion,
    code = "LN1369",
    tag = "out-of-date-standards-version",
    default_selected = false,
    experimental = false,
}

/// The vendored Policy release table, newest first: `version epoch`. Read only
/// for the ordered rows; the lookups are in the sibling module.
const POLICY_RELEASES: &str = include_str!("debian_policy_releases");

fn releases() -> Vec<(&'static str, i64)> {
    crate::lint::data::lines(POLICY_RELEASES)
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some((fields.next()?, fields.next()?.parse().ok()?))
        })
        .collect()
}

/// `^\d+\.\d+\.\d+(?:\.0)?$`
fn is_normative(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    let all_digits = |part: &&str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    match parts.as_slice() {
        [_, _, _] => parts.iter().all(all_digits),
        [_, _, _, "0"] => parts[..3].iter().all(all_digits),
        _ => false,
    }
}

fn date(epoch: i64) -> String {
    DateTime::from_timestamp(epoch, 0)
        .map(|time| time.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

impl SourceRule for OutOfDateStandardsVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(compliance) = standards_version(ctx) else {
            return;
        };
        if !is_three_part_number(&compliance) || !is_policy_release(&compliance) {
            return;
        }
        let Some(compliance_epoch) = policy_release_epoch(&compliance) else {
            return;
        };
        let latest_epoch = policy_release_epoch(latest_policy_version()).unwrap_or_default();
        let changelog_epoch = ctx
            .debian_changelog()
            .and_then(|changelog| changelog.entries().first())
            .and_then(|entry| entry.timestamp())
            .unwrap_or(latest_epoch);

        let table = releases();
        let newer: Vec<(&str, i64)> = table
            .iter()
            .copied()
            .take_while(|(_, epoch)| *epoch > compliance_epoch)
            .collect();
        let known_normative = newer
            .iter()
            .any(|(version, epoch)| is_normative(version) && *epoch <= changelog_epoch);
        if !known_normative {
            return;
        }
        let message = format!(
            "{compliance} (released {}) (current is {})",
            date(compliance_epoch),
            latest_policy_version_normalized()
        );
        ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Info));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(standards: &str, changelog_date: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: example\nMaintainer: Example <ex@example.com>\nStandards-Version: {standards}\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
                )),
            ),
            (
                "debian/changelog",
                TestFile::new(format!(
                    "example (1.0-1) unstable; urgency=medium\n\n  * Initial release.\n\n -- Example <ex@example.com>  {changelog_date}\n"
                )),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no control"))],
            // Predates every newer normative Policy release.
            tree("3.9.6", "Mon, 01 Sep 2014 10:00:00 +0000"),
            // The current Policy release is not out of date.
            tree("4.7.4", "Mon, 01 Jun 2026 10:00:00 +0000"),
            // Not a known release, so the newer / invalid Tags decide.
            tree("9.9.9", "Mon, 01 Jun 2026 10:00:00 +0000"),
        ];
        let fail = vec![tree("3.9.6", "Mon, 01 Jun 2026 10:00:00 +0000")];
        SourceTreeTester::new(OutOfDateStandardsVersion, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "3.9.6",
            "Mon, 01 Sep 2014 10:00:00 +0000",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "3.9.6",
            "Mon, 01 Jun 2026 10:00:00 +0000",
        ))];
        SourcePackageTester::new(OutOfDateStandardsVersion, pass, fail).test_and_snapshot();
    }

    #[test]
    fn normative_versions() {
        assert!(is_normative("4.7.4"));
        assert!(is_normative("4.7.4.0"));
        assert!(!is_normative("4.7.4.1"));
        assert!(!is_normative("4.7"));
    }
}
