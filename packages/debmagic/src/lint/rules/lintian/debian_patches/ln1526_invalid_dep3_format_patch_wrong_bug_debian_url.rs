use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep3_header::dep3_headers;

pub struct InvalidDep3FormatPatchWrongBugDebianUrl;

declare_rule! {
    /// A DEP-3 `Bug-Debian` field is not a Debian BTS or salsa.debian.org URL.
    ///
    /// Reads each patch under `debian/patches/` except `series` and `README`
    /// by its header, the text before the first line starting with `---`. Fires
    /// when `Bug-Debian` is set and is not an http or https URL whose host is
    /// `bugs.debian.org` or `salsa.debian.org` (case-insensitive), so `#123456`
    /// and other-host URLs fire. Extra is empty. Location is the patch. Info,
    /// not selected by default. A patch without Description or Subject, or
    /// without Origin, Author or From, is skipped. The host is cut from the URL
    /// by hand: lintian's URI parser is more lenient on malformed input, and
    /// that difference is not replicated.
    InvalidDep3FormatPatchWrongBugDebianUrl,
    code = "LN1526",
    tag = "invalid-dep3-format-patch-wrong-bug-debian-url",
    default_selected = false,
    experimental = false,
}

impl SourceRule for InvalidDep3FormatPatchWrongBugDebianUrl {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, header) in dep3_headers(ctx) {
            if header.bug_debian_is_wrong() {
                ctx.diagnostic(
                    Diagnostic::warn("")
                        .with_severity(Severity::Info)
                        .with_location(Location {
                            path,
                            line: None,
                            column: None,
                        }),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn patch(header: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/patches/fix.patch",
            TestFile::new(format!("{header}---\n\n--- a/foo\n+++ b/foo\n")),
        )]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            patch("Description: fix\nOrigin: upstream, commit:0123abcd\n"),
            // An empty value is not a wrong URL.
            patch("Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian:\n"),
            patch(
                "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: https://bugs.debian.org/123456\n",
            ),
            patch(
                "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: http://salsa.debian.org/pkg/-/issues/1\n",
            ),
            // Scheme, host and userinfo are case-insensitive or dropped; the port is ignored.
            patch(
                "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: HTTPS://BUGS.DEBIAN.ORG/1\n",
            ),
            patch(
                "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: https://user@bugs.debian.org:443/1\n",
            ),
            // Without Description and Subject the patch is skipped.
            patch("Origin: upstream, commit:0123abcd\nBug-Debian: #123456\n"),
            // The last mention counts.
            patch(
                "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: #1\nBug-Debian: https://bugs.debian.org/2\n",
            ),
            // Field names are case-insensitive.
            patch(
                "description: fix\nOrigin: upstream, commit:0123abcd\nbug-debian: https://bugs.debian.org/2\n",
            ),
            vec![(
                "debian/patches/series",
                TestFile::new("Bug-Debian: #123456\n"),
            )],
        ];
        let fail = vec![
            patch("Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: #123456\n"),
            patch(
                "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: https://bugs.example.org/1\n",
            ),
            patch(
                "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: ftp://bugs.debian.org/1\n",
            ),
            patch(
                "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: bugs.debian.org/1\n",
            ),
            patch(
                "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: https://bugs.debian.org.example.org/1\n",
            ),
            patch(
                "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: https://notbugs.debian.org/1\n",
            ),
            vec![
                (
                    "debian/patches/one.patch",
                    TestFile::new("Description: fix\nBug-Debian: #1\n---\n"),
                ),
                (
                    "debian/patches/two.patch",
                    TestFile::new("Subject: fix\nFrom: a\nBug-Debian: #2\n---\n"),
                ),
            ],
        ];
        SourceTreeTester::new(InvalidDep3FormatPatchWrongBugDebianUrl, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(patch(
            "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: https://bugs.debian.org/1\n",
        ))];
        let fail = vec![SourcePackageCase::from(patch(
            "Description: fix\nOrigin: upstream, commit:0123abcd\nBug-Debian: #1\n",
        ))];
        SourcePackageTester::new(InvalidDep3FormatPatchWrongBugDebianUrl, pass, fail)
            .test_and_snapshot();
    }
}
