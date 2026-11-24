use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct LatestDebianChangelogEntryWithoutNewDate;

declare_rule! {
    /// The newest Debian changelog entry is not dated after the one before it.
    ///
    /// Compares the trailer timestamps of the first two entries of
    /// `debian/changelog` (the parsed entries, so an entry with an unparseable
    /// date or without a trailer has none and is skipped). Fires when the
    /// newest is not later than the previous and its distribution is not
    /// `unreleased` in any letter case; `UNRELEASED unstable` is not exempt.
    /// Extra is empty; the heading line of the newest entry is the Location.
    /// Applies to the Source tree and Source package.
    LatestDebianChangelogEntryWithoutNewDate,
    code = "LN0026",
    tag = "latest-debian-changelog-entry-without-new-date",
    default_selected = true,
    experimental = false,
}

impl SourceRule for LatestDebianChangelogEntryWithoutNewDate {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(changelog) = ctx.debian_changelog() else {
            return;
        };
        let [latest, previous, ..] = changelog.entries() else {
            return;
        };
        let (Some(latest_time), Some(previous_time)) = (
            latest.timestamp().filter(|time| *time != 0),
            previous.timestamp().filter(|time| *time != 0),
        ) else {
            return;
        };
        if latest_time <= previous_time && !latest.distribution().eq_ignore_ascii_case("unreleased")
        {
            let line = latest.line();
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: PathBuf::from("debian/changelog"),
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn entry(version: &str, distribution: &str, date: &str) -> String {
        format!(
            "example ({version}) {distribution}; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  {date}\n"
        )
    }

    fn changelog(entries: &[String]) -> Vec<(&'static str, TestFile)> {
        vec![("debian/changelog", TestFile::new(entries.join("\n")))]
    }

    const OLD: &str = "Mon, 29 Dec 2008 17:34:02 +0000";
    const NEW: &str = "Tue, 30 Dec 2008 17:34:02 +0000";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            changelog(&[entry("1.0", "unstable", NEW)]),
            changelog(&[entry("2.0", "unstable", NEW), entry("1.0", "unstable", OLD)]),
            // Same date but unreleased.
            changelog(&[
                entry("2.0", "UNRELEASED", OLD),
                entry("1.0", "unstable", OLD),
            ]),
            changelog(&[
                entry("2.0", "unreleased", OLD),
                entry("1.0", "unstable", NEW),
            ]),
            // Only the first two entries are compared.
            changelog(&[
                entry("3.0", "unstable", NEW),
                entry("2.0", "unstable", OLD),
                entry("1.0", "unstable", NEW),
            ]),
            // An unparseable date has no timestamp.
            changelog(&[
                entry("2.0", "unstable", "The, 15 Apr 2004 23:33:51 +0200"),
                entry("1.0", "unstable", NEW),
            ]),
            // Different offsets, later instant.
            changelog(&[
                entry("2.0", "unstable", "Tue, 30 Dec 2008 17:34:02 +0100"),
                entry("1.0", "unstable", "Tue, 30 Dec 2008 17:34:02 +0200"),
            ]),
            vec![("debian/changelog", TestFile::new("not a changelog"))],
        ];
        let fail = vec![
            changelog(&[entry("2.0", "unstable", OLD), entry("1.0", "unstable", OLD)]),
            changelog(&[entry("2.0", "unstable", OLD), entry("1.0", "unstable", NEW)]),
            // The same instant in another offset.
            changelog(&[
                entry("2.0", "unstable", "Tue, 30 Dec 2008 17:34:02 +0100"),
                entry("1.0", "unstable", "Tue, 30 Dec 2008 16:34:02 +0000"),
            ]),
            // `UNRELEASED unstable` is not `unreleased`.
            changelog(&[
                entry("2.0", "UNRELEASED unstable", OLD),
                entry("1.0", "unstable", OLD),
            ]),
        ];
        SourceTreeTester::new(LatestDebianChangelogEntryWithoutNewDate, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(changelog(&[
            entry("2.0", "unstable", NEW),
            entry("1.0", "unstable", OLD),
        ]))];
        let fail = vec![SourcePackageCase::from(changelog(&[
            entry("2.0", "unstable", OLD),
            entry("1.0", "unstable", OLD),
        ]))];
        SourcePackageTester::new(LatestDebianChangelogEntryWithoutNewDate, pass, fail)
            .test_and_snapshot();
    }
}
