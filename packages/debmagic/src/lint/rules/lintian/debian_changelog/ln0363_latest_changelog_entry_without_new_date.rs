use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::installed_changelog;

pub struct LatestChangelogEntryWithoutNewDate;

declare_rule! {
    /// The newest installed changelog entry is not dated after the one before it.
    ///
    /// Reads the installed changelog the way `debian-changelog-has-wrong-day-of-week`
    /// does. Compares the trailer timestamps of the first two entries (an entry
    /// with an unparseable date or without a trailer has none and is skipped).
    /// Fires when the newest is not later than the previous and its distribution
    /// is not `unreleased` in any letter case; `UNRELEASED unstable` is not
    /// exempt. Extra is empty; the heading line of the newest entry is the
    /// Location. Udebs are not checked.
    LatestChangelogEntryWithoutNewDate,
    code = "LN0363",
    tag = "latest-changelog-entry-without-new-date",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LatestChangelogEntryWithoutNewDate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = installed_changelog(ctx) else {
            return;
        };
        let path = installed.path.clone();
        let [latest, previous, ..] = installed.parsed.entries() else {
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
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn entry(version: &str, distribution: &str, date: &str) -> String {
        format!(
            "example ({version}) {distribution}; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  {date}\n"
        )
    }

    fn changelog(entries: &[String]) -> String {
        entries.join("\n")
    }

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn packaged(name: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(name.to_string(), file)])
    }

    const OLD: &str = "Mon, 29 Dec 2008 17:34:02 +0000";
    const NEW: &str = "Tue, 30 Dec 2008 17:34:02 +0000";

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            packaged(
                "usr/share/doc/example/changelog.gz",
                gzip(&changelog(&[entry("1.0", "unstable", NEW)])),
            ),
            packaged(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&changelog(&[
                    entry("2.0", "unstable", NEW),
                    entry("1.0", "unstable", OLD),
                ])),
            ),
            packaged(
                "usr/share/doc/example/changelog",
                TestFile::new(changelog(&[
                    entry("2.0", "UNRELEASED", OLD),
                    entry("1.0", "unstable", OLD),
                ])),
            ),
            packaged(
                "usr/share/doc/example/changelog",
                TestFile::new(changelog(&[
                    entry("2.0", "unreleased", OLD),
                    entry("1.0", "unstable", NEW),
                ])),
            ),
            packaged(
                "usr/share/doc/example/changelog.gz",
                gzip(&changelog(&[
                    entry("2.0", "unstable", OLD),
                    entry("1.0", "unstable", OLD),
                ])),
            )
            .udeb(),
        ];
        let fail = vec![
            packaged(
                "usr/share/doc/example/changelog.gz",
                gzip(&changelog(&[
                    entry("2.0", "unstable", OLD),
                    entry("1.0", "unstable", OLD),
                ])),
            ),
            packaged(
                "usr/share/doc/example/changelog.Debian",
                TestFile::new(changelog(&[
                    entry("2.0", "unstable", OLD),
                    entry("1.0", "unstable", NEW),
                ])),
            ),
            packaged(
                "usr/share/doc/example/changelog",
                TestFile::new(changelog(&[
                    entry("2.0", "UNRELEASED unstable", OLD),
                    entry("1.0", "unstable", OLD),
                ])),
            ),
        ];
        BinaryPackageTester::new(LatestChangelogEntryWithoutNewDate, pass, fail)
            .test_and_snapshot();
    }
}
