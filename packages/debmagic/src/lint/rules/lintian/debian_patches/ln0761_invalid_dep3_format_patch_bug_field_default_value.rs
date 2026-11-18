use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep3_header::{BugProblem, dep3_headers};

pub struct InvalidDep3FormatPatchBugFieldDefaultValue;

declare_rule! {
    /// A DEP-3 `Bug` field still holds the template placeholder.
    ///
    /// Reads each patch under `debian/patches/` except `series` and `README`
    /// by its header, the text before the first line starting with `---`.
    /// Fires when `Bug` is `<url in upstream bugtracker>`. A patch without
    /// Description or Subject, or without Origin, Author or From, is skipped.
    /// Extra is empty. Location is the patch. Applies to the Source tree and
    /// Source package.
    InvalidDep3FormatPatchBugFieldDefaultValue,
    code = "LN0761",
    tag = "invalid-dep3-format-patch-bug-field-default-value",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidDep3FormatPatchBugFieldDefaultValue {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, header) in dep3_headers(ctx) {
            if header.bug_problem() == Some(BugProblem::DefaultValue) {
                ctx.diagnostic(Diagnostic::error("").with_location(Location {
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
            patch(
                "Description: fix\nAuthor: A <a@example.org>\nBug: https://example.org/issues/1\n",
            ),
            // Other Bug values are another Tag.
            patch("Description: fix\nOrigin: vendor\nBug: #927914\n"),
            patch("Description: fix\nOrigin: vendor\nBug: forgot_it\n"),
            // Without Description and Subject, or without Origin, Author and From.
            patch("Origin: vendor\nBug: <url in upstream bugtracker>\n"),
            patch("Description:\nOrigin: vendor\nBug: <url in upstream bugtracker>\n"),
            patch("Description: fix\nBug: <url in upstream bugtracker>\n"),
            // The last mention counts.
            patch(
                "Description: fix\nOrigin: vendor\nBug: <url in upstream bugtracker>\nBug: https://example.org/1\n",
            ),
            // An empty Bug is not a placeholder.
            patch("Description: fix\nOrigin: vendor\nBug:\n"),
            // Only patches are read.
            vec![(
                "debian/patches/series",
                TestFile::new("Bug: <url in upstream bugtracker>\n"),
            )],
        ];
        let fail = vec![
            patch("Description: fix\nOrigin: vendor\nBug: <url in upstream bugtracker>\n"),
            // Subject and Author satisfy the gates. Field names are case-insensitive.
            patch("subject: fix\nauthor: A <a@example.org>\nbug: <url in upstream bugtracker>\n"),
            // A later section with the placeholder counts.
            patch("Description: fix\nOrigin: vendor\n\nBug: <url in upstream bugtracker>\n"),
            vec![
                (
                    "debian/patches/one.patch",
                    TestFile::new(
                        "Description: fix\nFrom: a\nBug: <url in upstream bugtracker>\n---\n",
                    ),
                ),
                (
                    "debian/patches/two.patch",
                    TestFile::new(
                        "Description: fix\nFrom: a\nBug: <url in upstream bugtracker>\n---\n",
                    ),
                ),
            ],
        ];
        SourceTreeTester::new(InvalidDep3FormatPatchBugFieldDefaultValue, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(patch(
            "Description: fix\nOrigin: vendor\nBug: https://example.org/1\n",
        ))];
        let fail = vec![SourcePackageCase::from(patch(
            "Description: fix\nOrigin: vendor\nBug: <url in upstream bugtracker>\n",
        ))];
        SourcePackageTester::new(InvalidDep3FormatPatchBugFieldDefaultValue, pass, fail)
            .test_and_snapshot();
    }
}
