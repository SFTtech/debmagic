use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct GlobalFilesWildcardNotFirstParagraphInDep5Copyright;

declare_rule! {
    /// A DEP-5 `Files` paragraph other than the first after the header lists `*`.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. The wildcard must be exactly `*`. Extra is empty. The
    /// Location is the `Files` line.
    GlobalFilesWildcardNotFirstParagraphInDep5Copyright,
    code = "LN0615",
    tag = "global-files-wildcard-not-first-paragraph-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for GlobalFilesWildcardNotFirstParagraphInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            let Some((_header, followers)) = paragraphs.split_first() else {
                continue;
            };
            for paragraph in followers.iter().skip(1) {
                if !paragraph.trimmed_list("Files").contains(&"*") {
                    continue;
                }
                ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                    path: path.clone(),
                    line: paragraph.position("Files"),
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
            tree(
                "Files: a\nCopyright: 2020 Jane\nLicense: GPL-2\n\nFiles: b*\nCopyright: 2020 Jane\nLicense: MIT\n",
            ),
            tree(
                "Files: a\nCopyright: 2020 Jane\nLicense: GPL-2\n\nFiles: ./*\nCopyright: 2020 Jane\nLicense: MIT\n",
            ),
            tree("Copyright: 2020 Jane\nLicense: GPL-2\n"),
        ];
        let fail = vec![
            tree(
                "Files: a\nCopyright: 2020 Jane\nLicense: GPL-2\n\nFiles: *\nCopyright: 2020 Jane\nLicense: MIT\n",
            ),
            tree(
                "Files: a\nCopyright: 2020 Jane\nLicense: GPL-2\n\nFiles: b\n  *\nCopyright: 2020 Jane\nLicense: MIT\n\nFiles: *\nCopyright: 2021 Jane\nLicense: MIT\n",
            ),
        ];
        SourceTreeTester::new(
            GlobalFilesWildcardNotFirstParagraphInDep5Copyright,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
