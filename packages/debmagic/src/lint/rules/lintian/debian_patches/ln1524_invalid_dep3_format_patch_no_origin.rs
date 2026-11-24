use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep3_header::dep3_parsed;

pub struct InvalidDep3FormatPatchNoOrigin;

declare_rule! {
    /// A DEP-3 patch header has a `Description` or `Subject`, but no `Origin`,
    /// `Author` or `From` field with a value. The Location is the patch; the
    /// extra is empty.
    ///
    /// Reads each patch under `debian/patches/` as the DEP-3 header Rules do
    /// (see `LN0764`). A patch without a Description or Subject is left to
    /// `LN1523`, as lintian stops at that gate. Info severity and not selected
    /// by default.
    InvalidDep3FormatPatchNoOrigin,
    code = "LN1524",
    tag = "invalid-dep3-format-patch-no-origin",
    default_selected = false,
    experimental = false,
}

impl SourceRule for InvalidDep3FormatPatchNoOrigin {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let flagged: Vec<PathBuf> = dep3_parsed(ctx)
            .into_iter()
            .filter(|(_, header)| {
                header.has_description_or_subject() && !header.has_origin_author_or_from()
            })
            .map(|(path, _)| path)
            .collect();
        for path in flagged {
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
            patch("Description: fix\nOrigin: upstream, https://example.org/1\n"),
            patch("Description: fix\nAuthor: A <a@example.org>\n"),
            patch("Description: fix\nFrom: A <a@example.org>\n"),
            // Without a Description the first gate stops the Check.
            patch("Author: A <a@example.org>\n"),
        ];
        let fail = vec![
            patch("Description: fix\n"),
            // An empty Origin is no Origin.
            patch("Subject: fix\nOrigin:\n"),
        ];
        SourceTreeTester::new(InvalidDep3FormatPatchNoOrigin, pass, fail).test_and_snapshot();
    }

    /// A prose line in the header makes lintian's Deb822 reader die ("Cannot parse
    /// line"), and `Dep3.pm` returns before the Origin gate; a `#` line is skipped.
    #[test]
    fn unparseable_header_line_stops_the_check() {
        SourceTreeTester::new(
            InvalidDep3FormatPatchNoOrigin,
            vec![patch("Description: fix\n\nThe header text is in prose.\n")],
            vec![patch("Description: fix\n# a note\n")],
        )
        .test();
    }
}
