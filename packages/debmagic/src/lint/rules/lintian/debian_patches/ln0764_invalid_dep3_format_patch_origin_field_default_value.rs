use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep3_header::dep3_headers;

pub struct InvalidDep3FormatPatchOriginFieldDefaultValue;

declare_rule! {
    /// A DEP-3 `Origin` field still holds one of the template placeholders.
    ///
    /// Reads each patch under `debian/patches/` except `series` and `README`
    /// by its header, the text before the first line starting with `---`.
    /// Fires when `Origin` is `(upstream|backport|vendor|other), (<patch-url>|commit:<commit-id>)`
    /// or `<vendor|upstream|other>, <url of original patch>`. A patch without
    /// Description or Subject, or without Origin, Author or From, is skipped.
    /// Extra is empty. Location is the patch. Applies to the Source tree and
    /// Source package.
    InvalidDep3FormatPatchOriginFieldDefaultValue,
    code = "LN0764",
    tag = "invalid-dep3-format-patch-origin-field-default-value",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidDep3FormatPatchOriginFieldDefaultValue {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, header) in dep3_headers(ctx) {
            if header.origin_is_template() {
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
            patch("Description: fix\nOrigin: vendor, https://example.org/commit/1\n"),
            patch("Description: fix\nOrigin: upstream, commit:0123abcd\n"),
            // The templates need the full value, not a prefix.
            patch("Description: fix\nOrigin: (upstream|backport|vendor|other)\n"),
            patch("Description: fix\nOrigin: <vendor|upstream|other>\n"),
            // Without an Origin the Author or From satisfies the gate, and nothing fires.
            patch("Description: fix\nAuthor: A <a@example.org>\n"),
            // Without Description and Subject nothing fires.
            patch("Origin: vendor, <url of original patch>\n"),
            // The last mention counts.
            patch(
                "Description: fix\nOrigin: <vendor|upstream|other>, <url of original patch>\nOrigin: vendor, https://example.org/1\n",
            ),
            vec![(
                "debian/patches/series",
                TestFile::new("Origin: vendor, <url of original patch>\n"),
            )],
        ];
        let fail = vec![
            patch("Description: fix\nOrigin: <vendor|upstream|other>, <url of original patch>\n"),
            patch(
                "Description: fix\nOrigin: (upstream|backport|vendor|other), (<patch-url>|commit:<commit-id>)\n",
            ),
            // Subject and From satisfy the gates. Field names are case-insensitive.
            patch(
                "subject: fix\nfrom: a\norigin: <vendor|upstream|other>, <url of original patch>\n",
            ),
            vec![
                (
                    "debian/patches/one.patch",
                    TestFile::new(
                        "Description: fix\nFrom: a\nOrigin: <vendor|upstream|other>, <url of original patch>\n---\n",
                    ),
                ),
                (
                    "debian/patches/two.patch",
                    TestFile::new(
                        "Description: fix\nFrom: a\nOrigin: <vendor|upstream|other>, <url of original patch>\n---\n",
                    ),
                ),
            ],
        ];
        SourceTreeTester::new(InvalidDep3FormatPatchOriginFieldDefaultValue, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(patch(
            "Description: fix\nOrigin: vendor, https://example.org/1\n",
        ))];
        let fail = vec![SourcePackageCase::from(patch(
            "Description: fix\nOrigin: <vendor|upstream|other>, <url of original patch>\n",
        ))];
        SourcePackageTester::new(InvalidDep3FormatPatchOriginFieldDefaultValue, pass, fail)
            .test_and_snapshot();
    }
}
