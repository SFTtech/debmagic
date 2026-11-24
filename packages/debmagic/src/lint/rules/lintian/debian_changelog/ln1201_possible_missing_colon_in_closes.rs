use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PossibleMissingColonInCloses;

declare_rule! {
    /// The newest installed changelog entry says `closes` followed by a bug
    /// number but without the colon Policy requires.
    ///
    /// Reads the installed Debian changelog. A match is the word `closes`,
    /// optional whitespace or semicolons, an optional `bug`, an optional `#`,
    /// and six or more digits, followed by a non-word character. The match as
    /// written is the extra; the heading line of the entry is the Location.
    /// Udebs are not checked. Source packages are not checked: lintian's
    /// source-side run of the same Check is not ported yet.
    PossibleMissingColonInCloses,
    code = "LN1201",
    tag = "possible-missing-colon-in-closes",
    default_selected = true,
    experimental = false,
}

static MISSING_COLON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(closes[\s;]*(?:bug)?#?\s?\d{6,})[^\w]").expect("valid regex")
});

impl BinaryPackageRule for PossibleMissingColonInCloses {
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
        let line = latest.line();
        let matches: Vec<String> = MISSING_COLON
            .captures_iter(latest.changes())
            .filter_map(|captures| captures.get(1))
            .map(|closes| closes.as_str().to_string())
            .collect();
        for closes in matches {
            ctx.diagnostic(Diagnostic::error(closes).with_location(Location {
                path: installed.path.clone(),
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

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
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            // The colon is there.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fix. (Closes: #555555)")),
            ),
            // Five digits: not a bug number.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fix. closes #55555 here.")),
            ),
            // No bug number after the word.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fix, which closes the issue.")),
            ),
            // Only the newest entry is read.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&format!(
                    "{}{}",
                    entry("1.0-2", "Nothing."),
                    entry("1.0-1", "Fix. closes #555555 ")
                )),
            ),
            // Udebs are not checked.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fix. closes #555555 here")),
            )
            .udeb(),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fix. Closes #555555 here.")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("1.0", "Fix. closes bug#555555, and closes 666666;")),
            ),
        ];
        BinaryPackageTester::new(PossibleMissingColonInCloses, pass, fail).test_and_snapshot();
    }
}
