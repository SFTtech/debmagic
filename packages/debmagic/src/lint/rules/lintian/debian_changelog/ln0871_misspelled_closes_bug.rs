use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::installed_changelog;

pub struct MisspelledClosesBug;

declare_rule! {
    /// The newest installed changelog entry says `Close: #nnnn` instead of `Closes:`.
    ///
    /// Reads the installed changelog (see `no-changelog` for the names tried).
    /// Fires when the newest entry's changes contain `Close:`, whitespace, `#`
    /// and digits, matched case-insensitively. Extra is that matched text, such
    /// as `Close: #666666`; the heading line of the newest entry is the Location.
    /// Udebs are not checked.
    MisspelledClosesBug,
    code = "LN0871",
    tag = "misspelled-closes-bug",
    default_selected = true,
    experimental = false,
}

static MISSPELLED_CLOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(Close:\s+#\d+)").expect("valid regex"));

impl BinaryPackageRule for MisspelledClosesBug {
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
        let Some(found) = MISSPELLED_CLOSE.find(latest.changes()) else {
            return;
        };
        let extra = found.as_str().to_string();
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
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fixed. (Closes: #666666)")),
            ),
            // Only the newest entry is checked.
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&format!(
                    "{}{}",
                    entry("1.0-2", "Second."),
                    entry("1.0-1", "First. Close: #666666")
                )),
            ),
            // Close without a bug number is not this Tag.
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("1.0-1", "Close: the door")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry("1.0-1", "Close: #666666")),
            )
            .udeb(),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fixed. Close: #666666")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("1.0-1", "Fixed. close:   #123456 and more")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&format!(
                    "{}{}",
                    entry("1.0-2", "Second. CLOSE:\t#7"),
                    entry("1.0-1", "First.")
                )),
            ),
        ];
        BinaryPackageTester::new(MisspelledClosesBug, pass, fail).test_and_snapshot();
    }
}
