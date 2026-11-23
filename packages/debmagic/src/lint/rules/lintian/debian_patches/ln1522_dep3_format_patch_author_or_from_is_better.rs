use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep3_header::dep3_headers;

pub struct Dep3FormatPatchAuthorOrFromIsBetter;

declare_rule! {
    /// A DEP-3 patch header has an email address in its `Origin` field. The
    /// `Author` or `From` field is the place for a person, so `Origin` should
    /// name a category and a URL instead. The Location is the patch; the
    /// extra is empty.
    ///
    /// Reads each patch under `debian/patches/` as the DEP-3 header Rules do
    /// (see `LN0764`). The template values of `Origin` are not reported here,
    /// and a patch the Check stops at (no Description or Subject, or no
    /// Origin, Author or From) is skipped. Info severity and not selected by
    /// default.
    Dep3FormatPatchAuthorOrFromIsBetter,
    code = "LN1522",
    tag = "dep3-format-patch-author-or-from-is-better",
    default_selected = false,
    experimental = false,
}

/// lintian's email pattern for the `Origin` value.
static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").expect("valid regex")
});

impl SourceRule for Dep3FormatPatchAuthorOrFromIsBetter {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, header) in dep3_headers(ctx) {
            if header.origin_is_template() || !EMAIL.is_match(header.origin()) {
                continue;
            }
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

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

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
            patch("Description: fix\nOrigin: upstream, https://example.org/commit/1\n"),
            patch("Description: fix\nOrigin: vendor, commit:0123abcd\n"),
            // No Origin, only an Author with an email address.
            patch("Description: fix\nAuthor: A <a@example.org>\n"),
            // The Check stops before the Origin check without a Description or Subject.
            patch("Origin: Foo <foo@example.org>\n"),
            // An email address without a dot-domain is not one.
            patch("Description: fix\nOrigin: user@localhost\n"),
        ];
        let fail = vec![
            patch("Description: fix\nOrigin: Foo <foo@example.org>\n"),
            // The last mention counts.
            patch(
                "Description: fix\nOrigin: vendor, https://example.org/1\nOrigin: Foo <foo@example.org>\n",
            ),
        ];
        SourceTreeTester::new(Dep3FormatPatchAuthorOrFromIsBetter, pass, fail).test_and_snapshot();
    }
}
