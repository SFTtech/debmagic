use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::installed_changelog;

pub struct ChangelogIsDhMakeTemplate;

declare_rule! {
    /// The only installed changelog entry still has dh_make's ITP placeholder.
    ///
    /// Reads the installed changelog (see `no-changelog` for the names tried).
    /// Fires when the changelog has exactly one entry and its changes contain
    /// dh_make's `<nnnn> is the bug number of your ITP` text, matched
    /// case-insensitively with an optional `#` and `n` or digits before it.
    /// Extra is empty; the heading line of that entry is the Location. Changelogs
    /// with more than one entry are not checked. Udebs are not checked.
    ChangelogIsDhMakeTemplate,
    code = "LN0865",
    tag = "changelog-is-dh_make-template",
    default_selected = true,
    experimental = false,
}

static ITP_PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:#?\s*)(?:\d|n)+ is the bug number of your ITP").expect("valid regex")
});

impl BinaryPackageRule for ChangelogIsDhMakeTemplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = installed_changelog(ctx) else {
            return;
        };
        let [latest] = installed.parsed.entries() else {
            return;
        };
        if !ITP_PLACEHOLDER.is_match(latest.changes()) {
            return;
        }
        let line = latest.line();
        ctx.diagnostic(Diagnostic::error("").with_location(Location {
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
                gzip(&entry("1.0-1", "Initial release. (Closes: #123456)")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&format!(
                    "{}{}",
                    entry("1.0-2", "Second, <nnnn is the bug number of your ITP>."),
                    entry(
                        "1.0-1",
                        "Initial release (Closes: #nnnn)  <nnnn is the bug number of your ITP>."
                    )
                )),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("1.0-1", "Initial release. (Closes: #123456)")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry(
                    "1.0-1",
                    "Initial release (Closes: #nnnn) <nnnn is the bug number of your ITP>.",
                )),
            )
            .udeb(),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry(
                    "1.0-1",
                    "Initial release (Closes: #nnnn)  <nnnn is the bug number of your ITP>.",
                )),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry(
                    "1.0-1",
                    "Initial release. <123 IS THE BUG NUMBER OF YOUR ITP>",
                )),
            ),
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc/example/foo".to_string(),
                    TestFile::new(entry(
                        "1.0-1",
                        "Initial release. <n is the bug number of your ITP>",
                    )),
                ),
                (
                    "usr/share/doc/example/changelog".to_string(),
                    TestFile::symlink("foo"),
                ),
            ]),
        ];
        BinaryPackageTester::new(ChangelogIsDhMakeTemplate, pass, fail).test_and_snapshot();
    }
}
