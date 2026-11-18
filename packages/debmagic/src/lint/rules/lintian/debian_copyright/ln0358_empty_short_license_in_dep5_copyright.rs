use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5::license_identifier;

pub struct EmptyShortLicenseInDep5Copyright;

declare_rule! {
    /// A DEP-5 `License` field has an empty short name.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable, including the header. The first line is the short
    /// name; `(),` become spaces. A license body on the following lines does
    /// not fill the short name in. Extra is empty. The Location is the
    /// `License` line.
    EmptyShortLicenseInDep5Copyright,
    code = "LN0358",
    tag = "empty-short-license-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for EmptyShortLicenseInDep5Copyright {
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
                if !license_identifier(paragraph.untrimmed_value("License")).is_empty() {
                    continue;
                }
                let line = paragraph
                    .position("License")
                    .unwrap_or(paragraph.start_line());
                ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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

    #[test]
    fn test() {
        let pass = vec![
            tree("Files: *\nCopyright: 2020 Jane\nLicense: Expat\n"),
            tree("License: GPL-2+\n"),
            vec![("debian/copyright", TestFile::new("Copyright 2020 Jane\n"))],
            Vec::new(),
        ];
        let fail = vec![
            tree("License:\n"),
            tree("License:\n This package is free software.\n"),
            tree("License: (,)\n body\n"),
            vec![(
                "debian/copyright",
                TestFile::new(format!(
                    "{FORMAT}License:\n\nFiles: *\nCopyright: x\nLicense: MIT\n"
                )),
            )],
        ];
        SourceTreeTester::new(EmptyShortLicenseInDep5Copyright, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "Files: *\nCopyright: 2020 Jane\nLicense: Expat\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree("License:\n"))];
        SourcePackageTester::new(EmptyShortLicenseInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
