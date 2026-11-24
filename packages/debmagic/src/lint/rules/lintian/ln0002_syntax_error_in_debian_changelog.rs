use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_changelog::DebianChangelog;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct SyntaxErrorInDebianChangelog;

declare_rule! {
    /// Debian changelog grammar errors from parsing the installed changelog.
    ///
    /// Each parser error is one Diagnostic. Extra is the quoted condition
    /// lintian puts on `syntax-error-in-debian-changelog`. Lintian reports this
    /// Tag only for a Binary package, from its installed changelog (see
    /// `no-changelog` for the file names tried); a Source package's
    /// `debian/changelog` is never reported. A Binary package is skipped when
    /// `usr/share/doc/<package>` is a symlink, as lintian does. Location is the
    /// changelog file.
    SyntaxErrorInDebianChangelog,
    code = "LN0002",
    tag = "syntax-error-in-debian-changelog",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SyntaxErrorInDebianChangelog {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let doc_dir = format!("usr/share/doc/{}", ctx.package_name());
        if let Some(files) = ctx.data_files()
            && files.get(&doc_dir).is_some_and(|entry| entry.is_symlink())
        {
            return;
        }
        let Some(installed) = ctx.installed_changelog() else {
            return;
        };
        for (line, message) in reports(&installed.parsed) {
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path: installed.path.clone(),
                line: Some(line),
                column: None,
            }));
        }
    }
}

/// One `(line, extra)` per parser error; the extra is the quoted condition.
/// An unparseable date is the one condition lintian prefixes with the line
/// (`line 12 "couldn't parse date ..."`).
fn reports(changelog: &DebianChangelog) -> Vec<(u32, String)> {
    changelog
        .errors()
        .iter()
        .map(|error| {
            let extra = if error.message().starts_with("couldn't parse date ") {
                format!("line {} \"{}\"", error.line(), error.message())
            } else {
                format!("\"{}\"", error.message())
            };
            (error.line(), extra)
        })
        .collect()
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

    #[test]
    fn binary_package() {
        let valid = "example (1.0-1) unstable; urgency=low\n\n  * test\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 -0800\n";
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/changelog.Debian.gz".to_string(),
                gzip(valid),
            )]),
            // A symlinked doc directory is skipped, whatever it holds.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc/example".to_string(),
                    TestFile::symlink("elsewhere"),
                ),
                (
                    "usr/share/doc/example/changelog.Debian.gz".to_string(),
                    gzip("not a Debian changelog\n"),
                ),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/changelog.Debian.gz".to_string(),
                gzip("not a Debian changelog\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/changelog".to_string(),
                TestFile::new(
                    "example (1.0-1) unstable; urgency=low\n\n  * test\n\n -- A Uthor <a@example.com> Tue, 30 Dec 2008 17:34:02 -0800\n",
                ),
            )]),
        ];
        BinaryPackageTester::new(SyntaxErrorInDebianChangelog, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }

    /// Lintian's own parser cases: the grammar errors of an installed changelog.
    #[test]
    fn changelog_errors() {
        let fail = [
            "not a Debian changelog\n",
            concat!(
                "example (1.0) unstable; urgency=low\n",
                "\n",
                "  * .\n",
                "  *\n",
                "\n",
                " -- A Uthor <a@example.com> Tue, 30 Dec 2008 17:34:02 -0800\n",
                "\n",
                "example (0.1) unstable; urgency=low\n",
                "\n",
                " -- A Uthor <a@example.com>  Fri, 06 Feb 2009 22:22:37 -0800\n",
            ),
            concat!(
                "example (1.0) unstable; urgency=low\n",
                "\n",
                "  * Lintian Test Suite.\n",
                "  * Test: changelog-file-syntax\n",
                "\n",
                " -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 -0800\n",
                "\n",
                "example (0.9) unstable; urgency=low\n",
                "\n",
                "  * Lintian Test Suite.\n",
                "  * Test: changelog-file-syntax\n",
                "\n",
                " -- A Uthor <a@example.com>  Sat, 09 Apr 2016 10:56:49 +0000\n",
                "\n",
                "example () unstable; urgency=low\n",
                "\n",
                "  * Lintian Test Suite.\n",
                "  * Test: changelog-file-syntax\n",
                "\n",
                "  * Suppress \"should close ITP bug\" messages.  (Closes: #123456)\n",
                "\n",
                " -- A Uthor <a@example.com>  Sat, 02 Apr 2016 10:56:49 +0000\n",
            ),
            concat!(
                "example (1.0) unstable; urgency=low\n",
                "\n",
                "  * Lintian Test Suite.\n",
                "  * Test: changelog-file-strange-date\n",
                "\n",
                " -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 -0800\n",
                "\n",
                "example (1.0) unstable; urgency=low\n",
                "\n",
                "  * The date will fail with some dpkg version see #794674\n",
                "\n",
                " -- A Uthor <a@example.com>  The, 15 Apr 2004 23:33:51 +0200\n",
            ),
        ];
        let cases: Vec<BinaryPackageCase> = fail
            .into_iter()
            .map(|text| {
                BinaryPackageCase::from(vec![(
                    "usr/share/doc/example/changelog.Debian.gz".to_string(),
                    gzip(text),
                )])
            })
            .collect();
        BinaryPackageTester::new(SyntaxErrorInDebianChangelog, Vec::new(), cases)
            .test_and_snapshot_with_suffix("errors");
    }
}
