use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_changelog::DebianChangelog;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::installed_changelog;

pub struct InvalidDateInDebianChangelog;

declare_rule! {
    /// The newest installed changelog date spells out a weekday or month.
    ///
    /// Reads the installed changelog the way `debian-changelog-has-wrong-day-of-week`
    /// does. Each match from lintian's `changelog-file/invalid-dates` is
    /// reported when the date parses, including after those words are shortened
    /// the way `Date::Parse` accepts them. Extra is `(Match -> Short)`. The
    /// heading line is the Location. This is the `.deb` check, so udebs are
    /// not checked.
    InvalidDateInDebianChangelog,
    code = "LN0441",
    tag = "invalid-date-in-debian-changelog",
    default_selected = true,
    experimental = false,
}

const INVALID: &[(&str, &str)] = &[
    (r"Monday", "Mon"),
    (r"Tues(day)?", "Tue"),
    (r"Wednesday", "Wed"),
    (r"Thurs(day)?", "Thu"),
    (r"Friday", "Fri"),
    (r"Saturday", "Sat"),
    (r"Sunday", "Sun"),
    (r"January", "Jan"),
    (r"February", "Feb"),
    (r"March", "Mar"),
    (r"April", "Apr"),
    (r"June", "Jun"),
    (r"July", "Jul"),
    (r"August", "Aug"),
    (r"Sept(ember)?", "Sep"),
    (r"October", "Oct"),
    (r"November", "Nov"),
    (r"December", "Dec"),
];

impl BinaryPackageRule for InvalidDateInDebianChangelog {
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
        let date = latest.date();
        if date.is_empty() || !date_is_parsed(date) {
            return;
        }
        let path = installed.path.clone();
        let line = latest.line();
        for extra in invalid_date_hits(date) {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path: path.clone(),
                line: Some(line),
                column: None,
            }));
        }
    }
}

fn invalid_date_hits(date: &str) -> Vec<String> {
    patterns()
        .iter()
        .filter_map(|(pattern, replacement)| {
            pattern
                .find(date)
                .map(|found| format!("({} -> {replacement})", found.as_str()))
        })
        .collect()
}

fn date_is_parsed(date: &str) -> bool {
    if changelog_date_parses(date) {
        return true;
    }
    let mut normalized = date.to_string();
    for (pattern, replacement) in patterns() {
        normalized = pattern.replace_all(&normalized, *replacement).into_owned();
    }
    changelog_date_parses(&normalized)
}

fn changelog_date_parses(date: &str) -> bool {
    let text = format!(
        "example (1.0-1) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  {date}\n"
    );
    DebianChangelog::parse(&text)
        .entries()
        .first()
        .and_then(|entry| entry.timestamp())
        .is_some_and(|time| time != 0)
}

fn patterns() -> &'static Vec<(Regex, &'static str)> {
    static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
        INVALID
            .iter()
            .map(|(pattern, replacement)| {
                (
                    Regex::new(&format!("(?i){pattern}")).expect("invalid date pattern"),
                    *replacement,
                )
            })
            .collect()
    });
    &PATTERNS
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn changelog(date: &str) -> BinaryPackageCase {
        let text = format!(
            "example (1.0) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  {date}\n"
        );
        BinaryPackageCase::from(vec![("usr/share/doc/example/changelog.gz", gzip(&text))])
    }

    #[test]
    fn test() {
        let pass = vec![
            changelog("Tue, 14 Nov 2017 10:47:43 +0000"),
            changelog("not a date"),
            changelog("Thurs, 14 Sept 2017 10:47:43 +0000").udeb(),
        ];
        let fail = vec![
            changelog("Thurs, 14 Sept 2017 10:47:43 +0000"),
            changelog("Monday, 05 Nov 2013 23:59:59 +0000"),
        ];
        BinaryPackageTester::new(InvalidDateInDebianChangelog, pass, fail).test_and_snapshot();
    }
}
