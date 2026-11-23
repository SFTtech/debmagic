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
}
