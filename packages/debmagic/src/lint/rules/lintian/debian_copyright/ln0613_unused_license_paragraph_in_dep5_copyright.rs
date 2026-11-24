use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use crate::lint::debian_copyright::CopyrightParagraph;

use super::dep5::{found_standalone, license_identifier, license_names, license_text};

pub struct UnusedLicenseParagraphInDep5Copyright;

declare_rule! {
    /// A stand-alone DEP-5 license paragraph is not used by any `Files` paragraph.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. A `Files` paragraph without a license body requires each
    /// short name in its `License` line. A stand-alone paragraph (a `License` with
    /// a body and no `Files`) whose whole lowercased first line is not required
    /// is unused; `public-domain` is exempt. Extra is the name. The Location is
    /// the `License` line. Lintian rates this info, so it is not selected by
    /// default.
    UnusedLicenseParagraphInDep5Copyright,
    code = "LN0613",
    tag = "unused-license-paragraph-in-dep5-copyright",
    default_selected = false,
    experimental = false,
}

/// Short names required by the `Files` paragraphs after the header that have
/// no license body, as `required_standalone` collects them.
pub(super) fn follower_license_names(paragraphs: &[CopyrightParagraph]) -> Vec<(String, usize)> {
    let mut names = Vec::new();
    for (index, paragraph) in paragraphs.iter().enumerate().skip(1) {
        if !paragraph.declares("Files") || !paragraph.declares("License") {
            continue;
        }
        let raw = paragraph.untrimmed_value("License");
        if !license_text(raw).is_empty() {
            continue;
        }
        names.extend(
            license_names(&license_identifier(raw))
                .into_iter()
                .map(|name| (name, index)),
        );
    }
    names
}

impl SourceRule for UnusedLicenseParagraphInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            let required = follower_license_names(paragraphs);
            for (name, indexes) in found_standalone(paragraphs) {
                if required.iter().any(|(required, _)| *required == name) || name == "public-domain"
                {
                    continue;
                }
                for index in indexes {
                    ctx.diagnostic(
                        Diagnostic::warn(name.clone())
                            .with_severity(Severity::Info)
                            .with_location(Location {
                                path: path.clone(),
                                line: paragraphs[index].position("License"),
                                column: None,
                            }),
                    );
                }
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
            tree(
                "Files: *\nCopyright: 2020 Jane\nLicense: MIT or GPL-2\n\nLicense: mit\n One.\n\nLicense: GPL-2\n Two.\n",
            ),
            tree("License: Public-Domain\n Nothing uses this.\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: MIT\n"),
        ];
        let fail = vec![
            tree("Files: *\nCopyright: 2020 Jane\nLicense: MIT\n\nLicense: Other\n Full text.\n"),
            tree(
                "Files: *\nCopyright: 2020 Jane\nLicense: MIT or GPL-2\n\nLicense: MIT or GPL-2\n Full text.\n",
            ),
            tree(
                "Files: *\nCopyright: 2020 Jane\nLicense: MIT\n Inline text.\n\nLicense: MIT\n Full text.\n",
            ),
        ];
        SourceTreeTester::new(UnusedLicenseParagraphInDep5Copyright, pass, fail)
            .test_and_snapshot();
    }
}
