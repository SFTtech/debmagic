use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct AmbiguousParagraphInDep5Copyright;

declare_rule! {
    /// A DEP-5 paragraph after the header has `License` and `Copyright` but no `Files`.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. The header paragraph is not judged. Extra is empty. The
    /// Location is the first line of the paragraph.
    AmbiguousParagraphInDep5Copyright,
    code = "LN0611",
    tag = "ambiguous-paragraph-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for AmbiguousParagraphInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            let Some((_header, followers)) = paragraphs.split_first() else {
                continue;
            };
            for paragraph in followers.iter().filter(|paragraph| {
                paragraph.declares("License")
                    && paragraph.declares("Copyright")
                    && !paragraph.declares("Files")
            }) {
                ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                    path: path.clone(),
                    line: Some(paragraph.start_line()),
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
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-2\n"),
            tree("License: GPL-2\n Full text.\n"),
            tree("Copyright: 2020 Jane\n"),
            vec![(
                "debian/copyright",
                TestFile::new(format!("{FORMAT}Copyright: 2020 Jane\nLicense: GPL-2\n")),
            )],
            vec![(
                "debian/copyright",
                TestFile::new("Copyright: 2020 Jane\nLicense: GPL-2\n"),
            )],
        ];
        let fail = vec![
            tree("Copyright: 2020 Jane\nLicense: GPL-2\n"),
            tree(
                "Files: a\nCopyright: 2020 Jane\nLicense: GPL-2\n\nCopyright: 2020 Jane\nLicense: MIT\n\nCopyright: 2021 Jane\nLicense: MIT\n",
            ),
        ];
        SourceTreeTester::new(AmbiguousParagraphInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
