use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::installed::installed_news;

pub struct DebianNewsEntryUsesAsterisk;

declare_rule! {
    /// The newest `NEWS.Debian` entry presents its changes as a bulleted list:
    /// its text starts with an asterisk (after blanks) and a space. Debian
    /// Developer's Reference 6.3.4 asks for paragraphs instead. The Location is
    /// the heading line of the entry; the extra is empty.
    ///
    /// Reads `usr/share/doc/<pkg>/NEWS.Debian.gz` (that capitalization), as
    /// `LN0364` does. Only the start of the entry's text is looked at: a
    /// later asterisk line does not count, as in lintian's `^` without `/m`.
    /// Info severity and not selected by default. Udebs are not checked.
    DebianNewsEntryUsesAsterisk,
    code = "LN1517",
    tag = "debian-news-entry-uses-asterisk",
    default_selected = false,
    experimental = false,
}

static BULLET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\*\s").expect("valid regex"));

impl BinaryPackageRule for DebianNewsEntryUsesAsterisk {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = installed_news(ctx) else {
            return;
        };
        let Some(latest) = installed.parsed.entries().first() else {
            return;
        };
        if !BULLET.is_match(latest.changes()) {
            return;
        }
        let line = latest.line();
        ctx.diagnostic(
            Diagnostic::warn("")
                .with_severity(Severity::Info)
                .with_location(Location {
                    path: installed.path,
                    line: Some(line),
                    column: None,
                }),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn news(changes: &str) -> String {
        format!(
            "example (1.0) unstable; urgency=low\n\n{changes}\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
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
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            // Paragraphs, not a bulleted list.
            packaged(
                "usr/share/doc/example/NEWS.Debian.gz",
                gzip(&news("  The new release changes the default.")),
            ),
            // An asterisk that is not a bullet: no space after it.
            packaged(
                "usr/share/doc/example/NEWS.Debian.gz",
                gzip(&news("  *emphasis* is not a bullet.")),
            ),
            // Only the start of the entry's text counts.
            packaged(
                "usr/share/doc/example/NEWS.Debian.gz",
                gzip(&news("  Intro text.\n  * a later bullet.")),
            ),
            // Not the file name lintian reads.
            packaged(
                "usr/share/doc/example/NEWS.Debian",
                TestFile::new(news("  * A bullet.")),
            ),
            // Udebs are not checked.
            packaged(
                "usr/share/doc/example/NEWS.Debian.gz",
                gzip(&news("  * A bullet.")),
            )
            .udeb(),
        ];
        let fail = vec![
            packaged(
                "usr/share/doc/example/NEWS.Debian.gz",
                gzip(&news("  * A bullet.\n  * Another bullet.")),
            ),
            packaged(
                "usr/share/doc/example/NEWS.Debian.gz",
                gzip(&news("   *   indented bullet.")),
            ),
        ];
        BinaryPackageTester::new(DebianNewsEntryUsesAsterisk, pass, fail).test_and_snapshot();
    }
}
