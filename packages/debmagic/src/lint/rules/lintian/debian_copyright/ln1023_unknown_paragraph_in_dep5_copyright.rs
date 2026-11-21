use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct UnknownParagraphInDep5Copyright;

declare_rule! {
    /// A paragraph after the header has neither a `License` nor a `Files` field.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` when they
    /// are machine-readable and parse. The header (the first paragraph) is never
    /// reported. Extra is empty; the Location is the line where the paragraph
    /// starts.
    UnknownParagraphInDep5Copyright,
    code = "LN1023",
    tag = "unknown-paragraph-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UnknownParagraphInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, copyright) in ctx.debian_copyrights() {
            let Some(followers) = copyright.paragraphs().and_then(|all| all.get(1..)) else {
                continue;
            };
            for paragraph in followers {
                if paragraph.declares("License") || paragraph.declares("Files") {
                    continue;
                }
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

    const FORMAT: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn copyright(text: &str) -> TestFile {
        TestFile::new(format!("{FORMAT}{text}"))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![(
                "debian/copyright",
                copyright("\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n"),
            )],
            // The header is not checked, even without `Files` or `License`.
            vec![(
                "debian/copyright",
                TestFile::new(
                    "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\nUpstream-Name: x\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n",
                ),
            )],
            // A stand-alone license paragraph is known.
            vec![(
                "debian/copyright",
                copyright(
                    "\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n\nLicense: GPL-2\n body\n",
                ),
            )],
            // Not parsed: an outdated draft is never read.
            vec![(
                "debian/copyright",
                TestFile::new(
                    "Format: http://svn.debian.org/wsvn/dep/web/deps/dep5.mdwn?op=file&rev=147\n\nUpstream-Name: x\n",
                ),
            )],
        ];
        let fail = vec![
            // A paragraph with only a `Copyright` field.
            vec![(
                "debian/copyright",
                copyright(
                    "\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n\nCopyright: 2020 Other\n",
                ),
            )],
            // A paragraph with only a `Comment` field, after several known ones.
            vec![(
                "debian/copyright",
                copyright(
                    "\nFiles: a\nCopyright: 2024 Someone\nLicense: MIT\n\nFiles: b\nCopyright: 2024\nLicense: MIT\n\nComment: nothing\n",
                ),
            )],
        ];
        SourceTreeTester::new(UnknownParagraphInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
