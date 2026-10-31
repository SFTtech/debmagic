use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5::{license_identifier, license_text};

const MINIMUM_CREATIVE_COMMONS_NEWLINES: usize = 20;

pub struct IncompleteCreativeCommonsLicense;

declare_rule! {
    /// A Creative Commons short name is followed by only the human-readable summary.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. The lowercased first line of `License` must start with
    /// `cc-`, the body must be non-empty, must not point at
    /// `usr/share/common-licenses/CC-`, and must contain fewer than 20
    /// newlines. Extra is that lowercased first line. The Location is the
    /// `License` line.
    IncompleteCreativeCommonsLicense,
    code = "LN0359",
    tag = "incomplete-creative-commons-license",
    default_selected = true,
    experimental = false,
}

impl SourceRule for IncompleteCreativeCommonsLicense {
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
                let value = paragraph.untrimmed_value("License");
                let identifier = license_identifier(value);
                let text = license_text(value);
                if !identifier.starts_with("cc-")
                    || text.is_empty()
                    || text.contains("usr/share/common-licenses/CC-")
                    || text.matches('\n').count() >= MINIMUM_CREATIVE_COMMONS_NEWLINES
                {
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
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const FORMAT: &str = "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\
         Upstream-Name: example\n";

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/copyright",
            TestFile::new(format!("{FORMAT}\n{body}")),
        )]
    }

    fn long_cc() -> String {
        let mut body = String::from("License: CC-BY-4.0\n");
        for index in 0..21 {
            body.push_str(&format!(" line {index} of the full license\n"));
        }
        body
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-2\n The GPL.\n"),
            tree("License: CC-BY-4.0\n"),
            tree(
                "License: CC-BY-4.0\n On Debian systems, see /usr/share/common-licenses/CC-BY-4.0.\n",
            ),
            tree(&long_cc()),
            vec![("debian/copyright", TestFile::new("Copyright 2020 Jane\n"))],
            Vec::new(),
        ];
        let fail = vec![
            tree(
                "Files: *\nCopyright: 2012 Someone\nLicense: CC-BY-3.0\n You are free:\n to Share.\n",
            ),
            tree("License: CC-BY-3.0 or MIT\n You are free.\n"),
        ];
        SourceTreeTester::new(IncompleteCreativeCommonsLicense, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(tree("License: CC-BY-4.0\n"))];
        let fail = vec![SourcePackageCase::from(tree(
            "License: CC-BY-3.0\n You are free.\n",
        ))];
        SourcePackageTester::new(IncompleteCreativeCommonsLicense, pass, fail).test_and_snapshot();
    }
}
