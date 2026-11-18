use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::{installed_changelog, wrong_weekday};

pub struct DebianChangelogHasWrongDayOfWeek;

declare_rule! {
    /// The newest installed changelog entry names the wrong weekday.
    ///
    /// Reads `usr/share/doc/<pkg>/changelog.Debian.gz` (or the earlier
    /// candidate lintian accepts), gunzipped when the resolved name ends in
    /// `.gz`. The trailer timestamp is formatted in the trailer offset.
    /// Spelled-out weekdays and months are not reported here. Extra is
    /// `YYYY-MM-DD was a Weekday`. The heading line is the Location. Udebs
    /// are not checked.
    DebianChangelogHasWrongDayOfWeek,
    code = "LN0310",
    tag = "debian-changelog-has-wrong-day-of-week",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebianChangelogHasWrongDayOfWeek {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = installed_changelog(ctx) else {
            return;
        };
        let Some(latest) = installed.parsed.entries().first() else {
            return;
        };
        let Some(timestamp) = latest.timestamp().filter(|time| *time != 0) else {
            return;
        };
        let Some(extra) = wrong_weekday(latest.date(), timestamp) else {
            return;
        };
        let line = latest.line();
        ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
            path: installed.path,
            line: Some(line),
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn changelog(version: &str, date: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  {date}\n"
        )
    }

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn packaged(name: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(name.to_string(), file)])
    }

    #[test]
    fn test() {
        let right = changelog("1.0-1", "Tue, 30 Dec 2008 17:34:02 +0000");
        let wrong = changelog("1.0", "Fri, 07 Apr 2016 17:00:00 +0000");
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            packaged("usr/share/doc/example/changelog.Debian.gz", gzip(&right)),
            packaged("usr/share/doc/example/changelog", TestFile::new(&right)),
            // A spelled-out weekday is an invalid date, not this Tag.
            packaged(
                "usr/share/doc/example/changelog",
                TestFile::new(changelog("1.0-1", "Friday, 07 Apr 2016 17:00:00 +0000")),
            ),
            packaged("usr/share/doc/example/changelog.gz", gzip(&wrong)).udeb(),
        ];
        let fail = vec![
            packaged("usr/share/doc/example/changelog.gz", gzip(&wrong)),
            packaged(
                "usr/share/doc/example/changelog.Debian",
                TestFile::new(changelog("1.0-1", "Wed, 14 Apr 2003 01:35:47 +0200")),
            ),
        ];
        BinaryPackageTester::new(DebianChangelogHasWrongDayOfWeek, pass, fail).test_and_snapshot();
    }
}
