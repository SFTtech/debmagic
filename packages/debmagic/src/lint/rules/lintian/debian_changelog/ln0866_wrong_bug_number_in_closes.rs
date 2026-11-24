use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct WrongBugNumberInCloses;

declare_rule! {
    /// A `closes:` bug reference has a non-numeric bug number.
    ///
    /// Scans every line of the installed changelog as raw text (lintian reads
    /// the file, gunzipped when it is `.gz`, and skips it when it is not valid
    /// UTF-8). Lintian's two `closes:` patterns are matched in order: a bug
    /// number that starts with an optional `bug`, `#` and digits, then a letter
    /// and word characters (`#nnnn`). The Rule fires when the captured number
    /// has a non-empty prefix. Extra is the captured bug text, such as `#nnnn`;
    /// the Location is the line. Udebs are not checked.
    WrongBugNumberInCloses,
    code = "LN0866",
    tag = "wrong-bug-number-in-closes",
    default_selected = true,
    experimental = false,
}

static CLOSES_NON_NUMERIC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)closes:\s*(((?:bug)?#?\s?\d*)[[:alpha:]]\w*)").expect("valid regex")
});

static CLOSES_LIST_NON_NUMERIC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)closes:\s*(?:bug)?#?\s?\d+(?:,\s*(?:bug)?#?\s?\d+)*,\s*(((?:bug)?#?\s?\d*)[[:alpha:]]\w*)")
        .expect("valid regex")
});

impl BinaryPackageRule for WrongBugNumberInCloses {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = ctx.installed_changelog() else {
            return;
        };
        let path: PathBuf = installed.path.clone();
        for (index, line) in installed.text.split('\n').enumerate() {
            let captures = CLOSES_NON_NUMERIC
                .captures(line)
                .or_else(|| CLOSES_LIST_NON_NUMERIC.captures(line));
            let Some(captures) = captures else {
                continue;
            };
            let prefix_nonempty = captures
                .get(2)
                .is_some_and(|found| !found.as_str().is_empty());
            if !prefix_nonempty {
                continue;
            }
            let bug = captures.get(1).map_or("", |found| found.as_str());
            ctx.diagnostic(Diagnostic::warn(bug).with_location(Location {
                path: path.clone(),
                line: Some(index as u32 + 1),
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
                gzip(&entry("1.0-1", "Initial release. (Closes: #123456)")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("1.0-1", "Fixed. (Closes: #123456, #654321)")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("1.0-1", "Fixed. closes: bug #12 and more text")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry("1.0-1", "Fixed. (Closes: #nnnn)")),
            )
            .udeb(),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&format!(
                    "{}{}",
                    entry("1.0-2", "Fixed. (Closes: #nnnn)"),
                    entry("1.0-1", "First.")
                )),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("1.0-1", "Fixed. (Closes: #123456, #nnnn)")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("1.0-1", "Fixed. Closes: bugnnn")),
            ),
        ];
        BinaryPackageTester::new(WrongBugNumberInCloses, pass, fail).test_and_snapshot();
    }
}
