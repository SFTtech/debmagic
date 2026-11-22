use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::spelling::{check_spelling, group_exceptions};

use super::installed::installed_changelog;

pub struct SpellingErrorInChangelog;

declare_rule! {
    /// The change lines of the newest installed changelog entry contain a
    /// misspelling from lintian's spelling lists.
    ///
    /// Reads the installed changelog (see `no-changelog` for the names tried).
    /// Lines that contain `spelling` or `typo` (case-sensitively) are left out,
    /// so that a changelog entry for a spelling fix does not report itself.
    /// The Location is the changelog file with no line; extra is the word and
    /// its correction. Repeated words, the spelling exceptions of the Binary
    /// package's group and multi-word misspellings are checked as lintian's
    /// `check_spelling` does. Udebs are checked too.
    SpellingErrorInChangelog,
    code = "LN1310",
    tag = "spelling-error-in-changelog",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SpellingErrorInChangelog {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(installed) = installed_changelog(ctx) else {
            return;
        };
        let Some(latest) = installed.parsed.entries().first() else {
            return;
        };
        let changes = without_spelling_lines(latest.changes());
        let exceptions = group_exceptions(ctx);
        for (found, correction) in check_spelling(&changes, &exceptions, true) {
            ctx.diagnostic(
                Diagnostic::warn(format!("{found} {correction}")).with_location(Location {
                    path: installed.path.clone(),
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

/// `s/^.*(?:spelling|typo).*\n//gm`: a line with a complete newline is dropped
/// when it contains `spelling` or `typo`; a last line without one stays.
fn without_spelling_lines(changes: &str) -> String {
    changes
        .split_inclusive('\n')
        .filter(|line| {
            !(line.ends_with('\n') && (line.contains("spelling") || line.contains("typo")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn entry(version: &str, body: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * {body}\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn file(name: &str, contents: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(name.to_string(), contents)])
    }

    #[test]
    fn strips_lowercase_spelling_and_typo_lines() {
        assert_eq!(
            without_spelling_lines(" * a speling\n * Spelling fix\n * a typo\n * tail"),
            " * a speling\n * Spelling fix\n * tail"
        );
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fixed the spelling of teh word.")),
            ),
            // Only the newest entry is checked.
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&format!(
                    "{}{}",
                    entry("1.0-2", "Second."),
                    entry("1.0-1", "First, with a speling error.")
                )),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("1.0-1", "Corrected correct text.")),
            ),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "This has a speling error.")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("1.0-1", "Fixed. Teh thing, teh thing.")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry("1.0-1", "Speling, speling and speling.")),
            )
            .udeb(),
        ];
        BinaryPackageTester::new(SpellingErrorInChangelog, pass, fail).test_and_snapshot();
    }
}
