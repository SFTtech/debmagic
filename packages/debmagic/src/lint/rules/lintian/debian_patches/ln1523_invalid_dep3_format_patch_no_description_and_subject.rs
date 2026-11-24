use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep3_header::dep3_parsed;

pub struct InvalidDep3FormatPatchNoDescriptionAndSubject;

declare_rule! {
    /// A DEP-3 patch header has neither a `Description` nor a `Subject` field
    /// (or both are empty). The Location is the patch; the extra is empty.
    ///
    /// Reads each patch under `debian/patches/` as the DEP-3 header Rules do
    /// (see `LN0764`), and only the patches that have a header at all. This is
    /// the first gate of lintian's Check: a patch that fails it is not looked
    /// at for anything else. Info severity and not selected by default.
    InvalidDep3FormatPatchNoDescriptionAndSubject,
    code = "LN1523",
    tag = "invalid-dep3-format-patch-no-description-and-subject",
    default_selected = false,
    experimental = false,
}

impl SourceRule for InvalidDep3FormatPatchNoDescriptionAndSubject {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let flagged: Vec<PathBuf> = dep3_parsed(ctx)
            .into_iter()
            .filter(|(_, header)| !header.has_description_or_subject())
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
            patch("Subject: fix\nOrigin: upstream, https://example.org/1\n"),
        ];
        let fail = vec![
            patch("Origin: upstream, https://example.org/1\n"),
            // An empty Description is no description.
            patch("Description:\nOrigin: upstream, https://example.org/1\n"),
            // Author alone passes the second gate, not the first.
            patch("Author: A <a@example.org>\n"),
        ];
        SourceTreeTester::new(InvalidDep3FormatPatchNoDescriptionAndSubject, pass, fail)
            .test_and_snapshot();
    }

    /// lintian's Deb822 reader skips `#` lines and then finds no paragraph, so
    /// `Dep3.pm` returns without a hint (`return unless @sections`).
    #[test]
    fn comment_only_header_has_no_paragraph() {
        SourceTreeTester::new(
            InvalidDep3FormatPatchNoDescriptionAndSubject,
            vec![patch("## fix.diff by A\n##\n## Fix the build.\n\n")],
            vec![patch(
                "# a comment\nOrigin: upstream, https://example.org/1\n",
            )],
        )
        .test();
    }

    /// A header line that is neither a field nor a continuation makes the Deb822
    /// reader die ("Cannot parse line"), and `Dep3.pm` returns without a hint.
    #[test]
    fn unparseable_header_line_stops_the_check() {
        SourceTreeTester::new(
            InvalidDep3FormatPatchNoDescriptionAndSubject,
            vec![
                patch("Origin: upstream, https://example.org/1\n\nSome prose.\n"),
                patch("Replace legacy plugins\n\n"),
            ],
            vec![
                patch("Origin: upstream, https://example.org/1\n"),
                patch("Origin: upstream, https://example.org/1\n# prose is a comment\n"),
            ],
        )
        .test();
    }

    /// A continuation line after a blank line has no open field, which the
    /// Deb822 reader rejects ("Continuation line not in paragraph").
    #[test]
    fn continuation_after_blank_line_stops_the_check() {
        SourceTreeTester::new(
            InvalidDep3FormatPatchNoDescriptionAndSubject,
            vec![patch(
                "Origin: upstream, https://example.org/1\n\n Subject: fix\n",
            )],
            vec![patch("Origin: upstream,\n https://example.org/1\n")],
        )
        .test();
    }

    /// A field name repeated in one paragraph is a Deb822 syntax error ("Duplicate
    /// field"), but a repeat in another paragraph is a valid last mention.
    #[test]
    fn repeated_field_in_one_paragraph_stops_the_check() {
        SourceTreeTester::new(
            InvalidDep3FormatPatchNoDescriptionAndSubject,
            vec![patch(
                "Origin: upstream, https://example.org/1\nOrigin: other\n",
            )],
            vec![
                patch("Origin: upstream, https://example.org/1\n\nOrigin: other\n"),
                patch("Origin: upstream, https://example.org/1\n\nAuthor: A <a@example.org>\n"),
            ],
        )
        .test();
    }
}
