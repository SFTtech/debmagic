use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5::found_standalone;
use super::ln0613_unused_license_paragraph_in_dep5_copyright::follower_license_names;

pub struct Dep5FileParagraphReferencesHeaderParagraph;

declare_rule! {
    /// A DEP-5 `Files` paragraph takes its license text from the header paragraph.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. A `Files` paragraph without a license body requires each
    /// short name in its `License` line. When the only stand-alone paragraph with
    /// that whole lowercased first line is the header, the last requiring
    /// paragraph is reported. Extra is the short name. The Location is the
    /// `Files` line.
    Dep5FileParagraphReferencesHeaderParagraph,
    code = "LN0614",
    tag = "dep5-file-paragraph-references-header-paragraph",
    default_selected = true,
    experimental = false,
}

impl SourceRule for Dep5FileParagraphReferencesHeaderParagraph {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            let required = follower_license_names(paragraphs);
            for (name, indexes) in found_standalone(paragraphs) {
                let Some((_, requiring)) = required.iter().rfind(|(required, _)| *required == name)
                else {
                    continue;
                };
                if indexes.iter().any(|&index| index != 0) {
                    continue;
                }
                ctx.diagnostic(Diagnostic::warn(name.clone()).with_location(Location {
                    path: path.clone(),
                    line: paragraphs[*requiring].position("Files"),
                    column: None,
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const FORMAT: &str = "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\
         Upstream-Name: example\n";

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/copyright",
            TestFile::new(format!("{FORMAT}\n{body}")),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("Files: *\nCopyright: 2020 Jane\nLicense: MIT\n\nLicense: MIT\n Full text.\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: MIT\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: MIT\n Inline text.\n"),
        ];
        let fail = vec![
            vec![(
                "debian/copyright",
                TestFile::new(format!(
                    "{FORMAT}License: mit\n Header text.\n\nFiles: *\nCopyright: 2020 Jane\nLicense: MIT\n"
                )),
            )],
            vec![(
                "debian/copyright",
                TestFile::new(format!(
                    "{FORMAT}License: MIT\n Header text.\n\nFiles: a\nCopyright: 2020 Jane\nLicense: MIT\n\nFiles: b\nCopyright: 2020 Jane\nLicense: MIT\n"
                )),
            )],
        ];
        SourceTreeTester::new(Dep5FileParagraphReferencesHeaderParagraph, pass, fail)
            .test_and_snapshot();
    }
}
