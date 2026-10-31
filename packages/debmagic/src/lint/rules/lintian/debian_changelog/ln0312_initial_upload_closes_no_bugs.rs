use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::{closes_any, installed_changelog};

pub struct InitialUploadClosesNoBugs;

declare_rule! {
    /// The only installed changelog entry looks like a first upload and
    /// closes no bugs.
    ///
    /// One entry whose version ends in `-1`, and whose changes have no
    /// `closes:` bug list. Extra is empty. The heading line of that entry is
    /// the Location. Udebs are not checked.
    InitialUploadClosesNoBugs,
    code = "LN0312",
    tag = "initial-upload-closes-no-bugs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitialUploadClosesNoBugs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = installed_changelog(ctx) else {
            return;
        };
        let entries = installed.parsed.entries();
        let [latest] = entries else {
            return;
        };
        if !latest.version().ends_with("-1") || closes_any(latest.changes()) {
            return;
        }
        let line = latest.line();
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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
                gzip(&entry("1.0-1", "Initial upload. (Closes: #123456)")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&format!(
                    "{}{}",
                    entry("1.0-2", "Second."),
                    entry("1.0-1", "First.")
                )),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("1.0", "Native first upload.")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry("1.0-1", "Initial upload.")),
            )
            .udeb(),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry("1-1", "Initial setup")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("2:1.0-1", "First revision.")),
            ),
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc/example/foo".to_string(),
                    TestFile::new(entry("1.0-1", "Initial setup")),
                ),
                (
                    "usr/share/doc/example/changelog".to_string(),
                    TestFile::symlink("foo"),
                ),
            ]),
        ];
        BinaryPackageTester::new(InitialUploadClosesNoBugs, pass, fail).test_and_snapshot();
    }
}
