use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5::{follower_wildcards, license_text};

pub struct MissingLicenseTextInDep5Copyright;

declare_rule! {
    /// A standalone DEP-5 license paragraph gives a short name and no license text.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. A follower with no `Files` patterns is standalone,
    /// except the first follower that has both `License` and `Copyright`: that
    /// one stands for `*`. Extra is the `License` value (the short name, when
    /// the body is empty). The Location is the `License` line.
    MissingLicenseTextInDep5Copyright,
    code = "LN0357",
    tag = "missing-license-text-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MissingLicenseTextInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            let Some((_header, followers)) = paragraphs.split_first() else {
                continue;
            };
            for (index, paragraph) in followers.iter().enumerate() {
                if !paragraph.declares("License") {
                    continue;
                }
                let value = paragraph.untrimmed_value("License");
                if !follower_wildcards(paragraph, index).is_empty()
                    || !license_text(value).is_empty()
                {
                    continue;
                }
                let line = paragraph
                    .position("License")
                    .unwrap_or(paragraph.start_line());
                ctx.diagnostic(Diagnostic::warn(value).with_location(Location {
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
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-2\n"),
            tree("License: Expat\n Permission is granted.\n"),
            // The first License+Copyright follower without Files stands for `*`.
            tree("License: GPL-2\nCopyright: 2020 Jane\n"),
            tree("License:\n This package is free software.\n"),
            vec![("debian/copyright", TestFile::new("Copyright 2020 Jane\n"))],
            Vec::new(),
        ];
        let fail = vec![
            tree("License:\n"),
            tree("License: GPL-2+\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: Expat\n\nLicense: BSD-3\n"),
            tree("License: GPL-2\nCopyright: 2020 Jane\n\nLicense: GPL-3\n"),
        ];
        SourceTreeTester::new(MissingLicenseTextInDep5Copyright, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "License: Expat\n Permission is granted.\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree("License: GPL-3\n"))];
        SourcePackageTester::new(MissingLicenseTextInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
