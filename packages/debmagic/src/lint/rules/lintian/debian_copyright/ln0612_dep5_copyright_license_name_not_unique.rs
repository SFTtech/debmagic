use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5::found_standalone;

pub struct Dep5CopyrightLicenseNameNotUnique;

declare_rule! {
    /// Several stand-alone DEP-5 license paragraphs have the same short name.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. A stand-alone paragraph has a `License` with a body and
    /// no `Files`; the whole lowercased first line is the name, and
    /// `public-domain` is exempt. Extra is the name. The Location is the
    /// `License` line of each paragraph.
    Dep5CopyrightLicenseNameNotUnique,
    code = "LN0612",
    tag = "dep5-copyright-license-name-not-unique",
    default_selected = true,
    experimental = false,
}

impl SourceRule for Dep5CopyrightLicenseNameNotUnique {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            for (name, indexes) in found_standalone(paragraphs) {
                if indexes.len() < 2 || name == "public-domain" {
                    continue;
                }
                for index in indexes {
                    ctx.diagnostic(Diagnostic::warn(name.clone()).with_location(Location {
                        path: path.clone(),
                        line: paragraphs[index].position("License"),
                        column: None,
                    }));
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
            tree("License: MIT\n One.\n\nLicense: GPL-2\n Two.\n"),
            tree("License: Public-Domain\n One.\n\nLicense: public-domain\n Two.\n"),
            tree("License: MIT\n\nLicense: MIT\n"),
        ];
        let fail = vec![
            tree("License: MIT\n One.\n\nLicense: MIT\n Two.\n"),
            tree("License: MIT\n One.\n\nLicense: mit\n Two.\n\nLicense: MIT\n Three.\n"),
        ];
        SourceTreeTester::new(Dep5CopyrightLicenseNameNotUnique, pass, fail).test_and_snapshot();
    }
}
