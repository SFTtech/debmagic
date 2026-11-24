use std::sync::{LazyLock, OnceLock};

use regex::{Regex, RegexSet};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_changelog::DebianChangelog;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

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
        let Some(installed) = ctx.installed_changelog() else {
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
    matcher()
        .matches(date)
        .into_iter()
        .filter_map(|index| {
            let (_, replacement) = INVALID[index];
            compiled(index)
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
    for (index, (_, replacement)) in INVALID.iter().enumerate() {
        normalized = compiled(index)
            .replace_all(&normalized, *replacement)
            .into_owned();
    }
    changelog_date_parses(&normalized)
}

/// Set of all `INVALID` patterns, to find which of them a date contains
/// without compiling the others.
fn matcher() -> &'static RegexSet {
    static MATCHER: LazyLock<RegexSet> = LazyLock::new(|| {
        RegexSet::new(INVALID.iter().map(|(pattern, _)| format!("(?i){pattern}")))
            .expect("invalid date pattern")
    });
    &MATCHER
}

/// The `INVALID` pattern at `index`, compiled on first use. Most dates contain
/// none of the names, so most of the patterns are never compiled.
fn compiled(index: usize) -> &'static Regex {
    static SLOTS: LazyLock<Vec<OnceLock<Regex>>> =
        LazyLock::new(|| INVALID.iter().map(|_| OnceLock::new()).collect());
    SLOTS[index].get_or_init(|| {
        Regex::new(&format!("(?i){}", INVALID[index].0)).expect("invalid date pattern")
    })
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
    fn hits_name_each_spelled_out_name_once_with_its_abbreviation() {
        assert_eq!(
            invalid_date_hits("Monday, 05 Nov 2013"),
            ["(Monday -> Mon)"]
        );
        assert_eq!(invalid_date_hits("monday"), ["(monday -> Mon)"]);
        assert_eq!(invalid_date_hits("Tuesday"), ["(Tuesday -> Tue)"]);
        assert_eq!(
            invalid_date_hits("Thurs, 14 Sept 2017 10:47:43 +0000"),
            ["(Thurs -> Thu)", "(Sept -> Sep)"]
        );
        // No word boundaries: a name inside a longer word still counts.
        assert_eq!(invalid_date_hits("Mondays"), ["(Monday -> Mon)"]);
        // LATIN SMALL LETTER LONG S folds to `s` under Unicode case folding.
        assert_eq!(invalid_date_hits("\u{17f}unday"), ["(\u{17f}unday -> Sun)"]);
        assert!(invalid_date_hits("Tue, 14 Nov 2017 10:47:43 +0000").is_empty());
        assert!(invalid_date_hits("Sunshine").is_empty());
        assert!(invalid_date_hits("Mon, 06 Nov 2013").is_empty());
    }

    #[test]
    fn dates_parse_once_their_names_are_abbreviated() {
        assert!(date_is_parsed("Tue, 14 Nov 2017 10:47:43 +0000"));
        assert!(date_is_parsed("Thurs, 14 Sept 2017 10:47:43 +0000"));
        assert!(date_is_parsed("Monday, 05 Nov 2013 23:59:59 +0000"));
        assert!(!date_is_parsed("not a date"));
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
