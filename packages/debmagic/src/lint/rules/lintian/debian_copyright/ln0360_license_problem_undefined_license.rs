use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5::{license_identifier, license_names};

pub struct LicenseProblemUndefinedLicense;

declare_rule! {
    /// A DEP-5 short license is a placeholder or a bare hyphen.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. The first line of `License` is lowercased and split on
    /// `and` / `or`. A name that is `-`, or that contains `fixme`, `todo`,
    /// `undefine`/`undefined`, `unknow`/`unknown`, or `unspecified` as a word,
    /// is reported. Extra is that lowercased name. The Location is the
    /// `License` line.
    LicenseProblemUndefinedLicense,
    code = "LN0360",
    tag = "license-problem-undefined-license",
    default_selected = true,
    experimental = false,
}

static UNDEFINED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:fixmes?|todos?|undefined?|unknown?|unspecified)\b").expect("valid regex")
});

impl SourceRule for LicenseProblemUndefinedLicense {
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
                for name in license_names(&license_identifier(paragraph.untrimmed_value("License")))
                {
                    if name != "-" && !UNDEFINED.is_match(&name) {
                        continue;
                    }
                    ctx.diagnostic(Diagnostic::error(name).with_location(Location {
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
            tree("Files: *\nCopyright: 2020 Jane\nLicense: Expat\n text\n"),
            tree("License: common public license - v 1.0\n text\n"),
            tree("License: unknowns\n text\n"),
            tree("License: GPL-2\n"),
            vec![("debian/copyright", TestFile::new("Copyright 2020 Jane\n"))],
            Vec::new(),
        ];
        let fail = vec![
            tree("License: Fixme\n Fixme-license text\n"),
            tree("License: unknown\n text\n"),
            tree("License: unknow\n text\n"),
            tree("License: todo\n text\n"),
            tree("License: -\n text\n"),
            tree("License: undefined license\n text\n"),
            tree("License: undefined\n text\n"),
            tree("License: GPL-2 or TODO\n text\n"),
            tree("License: my-unknown-license\n text\n"),
            tree("License: undefine\n text\n"),
        ];
        SourceTreeTester::new(LicenseProblemUndefinedLicense, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(tree("License: Expat\n"))];
        let fail = vec![SourcePackageCase::from(tree("License: unknown\n text\n"))];
        SourcePackageTester::new(LicenseProblemUndefinedLicense, pass, fail).test_and_snapshot();
    }
}
