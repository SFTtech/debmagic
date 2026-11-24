use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5::license_identifier;

pub struct PipeSymbolUsedAsLicenseDisjunction;

declare_rule! {
    /// A DEP-5 license short name joins alternatives with a pipe.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. The first line of `License` is lowercased with `(),`
    /// turned into spaces; a pipe between whitespace is reported. Extra is that
    /// whole line. The Location is the `License` line.
    PipeSymbolUsedAsLicenseDisjunction,
    code = "LN0609",
    tag = "pipe-symbol-used-as-license-disjunction",
    default_selected = true,
    experimental = false,
}

static PIPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+\|\s+").expect("valid regex"));

impl SourceRule for PipeSymbolUsedAsLicenseDisjunction {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            for paragraph in paragraphs
                .iter()
                .filter(|paragraph| paragraph.declares("License"))
            {
                let identifier = license_identifier(paragraph.untrimmed_value("License"));
                if !PIPE.is_match(&identifier) {
                    continue;
                }
                let line = paragraph
                    .position("License")
                    .unwrap_or(paragraph.start_line());
                ctx.diagnostic(Diagnostic::warn(identifier).with_location(Location {
                    path: path.clone(),
                    line: Some(line),
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
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-2 or MIT\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: a|b\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-2\n | not a short name\n"),
            vec![(
                "debian/copyright",
                TestFile::new("Copyright 2020\nLicense: a | b\n"),
            )],
        ];
        let fail = vec![
            tree("Files: *\nCopyright: 2020 Jane\nLicense: This | That\n"),
            tree(
                "Files: a\nCopyright: 2020 Jane\nLicense: GPL-2 | MIT\n\nFiles: b\nCopyright: 2020 Jane\nLicense: (A | B), C\n",
            ),
        ];
        SourceTreeTester::new(PipeSymbolUsedAsLicenseDisjunction, pass, fail).test_and_snapshot();
    }
}
