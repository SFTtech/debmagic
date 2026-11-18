use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep3_header::{BugProblem, dep3_headers};

pub struct InvalidDep3FormatPatchBugNotContainUrl;

declare_rule! {
    /// A DEP-3 `Bug` field is neither an http or https URL nor a Debian bug number.
    ///
    /// Reads each patch under `debian/patches/` except `series` and `README`
    /// by its header, the text before the first line starting with `---`.
    /// Fires when `Bug` is set, its scheme is not http or https, and it is
    /// not `<url in upstream bugtracker>` or `#` followed by digits. A patch
    /// without Description or Subject, or without Origin, Author or From, is
    /// skipped. Extra is empty. Location is the patch. Applies to the Source
    /// tree and Source package.
    InvalidDep3FormatPatchBugNotContainUrl,
    code = "LN0762",
    tag = "invalid-dep3-format-patch-bug-not-contain-url",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidDep3FormatPatchBugNotContainUrl {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, header) in dep3_headers(ctx) {
            if header.bug_problem() == Some(BugProblem::NotUrl) {
                ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                    path,
                    line: None,
                    column: None,
                }));
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
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            patch("Description: fix\nOrigin: vendor\nBug: https://example.org/issues/1\n"),
            patch("Description: fix\nOrigin: vendor\nBug: HTTP://example.org/issues/1\n"),
            // Other Bug values are other Tags.
            patch("Description: fix\nOrigin: vendor\nBug: <url in upstream bugtracker>\n"),
            patch("Description: fix\nOrigin: vendor\nBug: #927914\n"),
            // An empty Bug is not set.
            patch("Description: fix\nOrigin: vendor\nBug:\n"),
            // Without Description and Subject, or without Origin, Author and From.
            patch("Origin: vendor\nBug: forgot_it\n"),
            patch("Description: fix\nBug: forgot_it\n"),
            // The last mention counts.
            patch("Description: fix\nOrigin: vendor\nBug: forgot_it\nBug: https://example.org/1\n"),
            vec![("debian/patches/series", TestFile::new("Bug: forgot_it\n"))],
        ];
        let fail = vec![
            patch("Description: fix\nOrigin: vendor\nBug: forgot_it\n"),
            patch("Description: fix\nOrigin: vendor\nBug: example.org/issues/1\n"),
            patch("Description: fix\nOrigin: vendor\nBug: ftp://example.org/issues/1\n"),
            // A bare hash is not a bug number.
            patch("Description: fix\nOrigin: vendor\nBug: #\n"),
            // Subject and From satisfy the gates. Field names are case-insensitive.
            patch("subject: fix\nfrom: a\nbug: forgot_it\n"),
            vec![
                (
                    "debian/patches/one.patch",
                    TestFile::new("Description: fix\nFrom: a\nBug: forgot_it\n---\n"),
                ),
                (
                    "debian/patches/two.patch",
                    TestFile::new("Description: fix\nFrom: a\nBug: forgot_it\n---\n"),
                ),
            ],
        ];
        SourceTreeTester::new(InvalidDep3FormatPatchBugNotContainUrl, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(patch(
            "Description: fix\nOrigin: vendor\nBug: https://example.org/1\n",
        ))];
        let fail = vec![SourcePackageCase::from(patch(
            "Description: fix\nOrigin: vendor\nBug: forgot_it\n",
        ))];
        SourcePackageTester::new(InvalidDep3FormatPatchBugNotContainUrl, pass, fail)
            .test_and_snapshot();
    }
}
