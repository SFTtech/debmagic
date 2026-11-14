use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5::{license_identifier, license_names};

pub struct InvalidShortNameInDep5Copyright;

declare_rule! {
    /// A DEP-5 license short name is a misspelling of a standard short name.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. The first line of `License` is lowercased and split on
    /// `and` / `or`. Extra is the short name that matches lintian's `gpl` /
    /// `lgpl` / `agpl` or `bsd` patterns. The Location is the `License` line.
    InvalidShortNameInDep5Copyright,
    code = "LN0442",
    tag = "invalid-short-name-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

static GPLISH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:agpl|gpl|lgpl)[^-]?\d(?:\.\d)?\+?$").expect("valid regex"));
static BSD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^bsd(?:[^-]?[234][^-]?(?:clause|cluase))?$").expect("valid regex")
});

impl SourceRule for InvalidShortNameInDep5Copyright {
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
                let line = paragraph
                    .position("License")
                    .unwrap_or(paragraph.start_line());
                let identifier = license_identifier(paragraph.untrimmed_value("License"));
                for name in license_names(&identifier) {
                    if !GPLISH.is_match(&name) && !BSD.is_match(&name) {
                        continue;
                    }
                    ctx.diagnostic(Diagnostic::warn(name).with_location(Location {
                        path: path.clone(),
                        line: Some(line),
                        column: None,
                    }));
                }
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
            tree("Files: *\nCopyright: 2020 Jane\nLicense: BSD-3-clause\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: Expat\n"),
            vec![("debian/copyright", TestFile::new("Copyright 2020 Jane\n"))],
        ];
        let fail = vec![
            tree("Files: *\nCopyright: 2020 Jane\nLicense: gplv1\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: bsd\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: LGPL2.1+\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: bsd2clause\n"),
        ];
        SourceTreeTester::new(InvalidShortNameInDep5Copyright, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "Files: *\nCopyright: 2020 Jane\nLicense: GPL-2\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "Files: *\nCopyright: 2020 Jane\nLicense: gplv1\n",
        ))];
        SourcePackageTester::new(InvalidShortNameInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
