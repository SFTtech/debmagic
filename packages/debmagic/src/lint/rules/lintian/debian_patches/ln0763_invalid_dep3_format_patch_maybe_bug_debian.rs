use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep3_header::{BugProblem, dep3_headers};

pub struct InvalidDep3FormatPatchMaybeBugDebian;

declare_rule! {
    /// A DEP-3 `Bug` field holds a Debian bug number instead of an upstream URL.
    ///
    /// Reads each patch under `debian/patches/` except `series` and `README`
    /// by its header, the text before the first line starting with `---`.
    /// Fires when `Bug` is `#` followed by digits only. A patch without
    /// Description or Subject, or without Origin, Author or From, is skipped.
    /// Extra is empty. Location is the patch. Applies to the Source tree and
    /// Source package.
    InvalidDep3FormatPatchMaybeBugDebian,
    code = "LN0763",
    tag = "invalid-dep3-format-patch-maybe-bug-debian",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidDep3FormatPatchMaybeBugDebian {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, header) in dep3_headers(ctx) {
            if header.bug_problem() == Some(BugProblem::DebianNumber) {
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
            // Other Bug values are other Tags.
            patch("Description: fix\nOrigin: vendor\nBug: forgot_it\n"),
            patch("Description: fix\nOrigin: vendor\nBug: <url in upstream bugtracker>\n"),
            // A Debian bug number needs the hash and digits only.
            patch("Description: fix\nOrigin: vendor\nBug: #\n"),
            patch("Description: fix\nOrigin: vendor\nBug: #92a7\n"),
            patch("Description: fix\nOrigin: vendor\nBug: 927914\n"),
            // Without Description and Subject, or without Origin, Author and From.
            patch("Origin: vendor\nBug: #927914\n"),
            patch("Description: fix\nBug: #927914\n"),
            // The last mention counts.
            patch("Description: fix\nOrigin: vendor\nBug: #927914\nBug: https://example.org/1\n"),
            vec![("debian/patches/series", TestFile::new("#927914\n"))],
        ];
        let fail = vec![
            patch("Description: fix\nOrigin: vendor\nBug: #927914\n"),
            patch("Description: fix\nOrigin: vendor\nBug: #1\n"),
            // Subject and Author satisfy the gates. Field names are case-insensitive.
            patch("subject: fix\nauthor: A <a@example.org>\nbug: #927914\n"),
            vec![
                (
                    "debian/patches/one.patch",
                    TestFile::new("Description: fix\nFrom: a\nBug: #927914\n---\n"),
                ),
                (
                    "debian/patches/two.patch",
                    TestFile::new("Description: fix\nFrom: a\nBug: #927914\n---\n"),
                ),
            ],
        ];
        SourceTreeTester::new(InvalidDep3FormatPatchMaybeBugDebian, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(patch(
            "Description: fix\nOrigin: vendor\nBug: https://example.org/1\n",
        ))];
        let fail = vec![SourcePackageCase::from(patch(
            "Description: fix\nOrigin: vendor\nBug: #927914\n",
        ))];
        SourcePackageTester::new(InvalidDep3FormatPatchMaybeBugDebian, pass, fail)
            .test_and_snapshot();
    }
}
