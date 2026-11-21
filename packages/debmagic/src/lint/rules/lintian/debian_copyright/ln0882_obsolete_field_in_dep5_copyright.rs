use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct ObsoleteFieldInDep5Copyright;

declare_rule! {
    /// A machine-readable copyright header uses a field that was renamed.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` (symlinks
    /// and files that are not UTF-8 are skipped). Only the header paragraph of
    /// a machine-readable file is checked, for `Format-Specification`,
    /// `Maintainer`, `Upstream-Maintainer`, `Contact` and `Name`. Extra is the
    /// old field, a space, and its replacement; the Location is the old field.
    ObsoleteFieldInDep5Copyright,
    code = "LN0882",
    tag = "obsolete-field-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for ObsoleteFieldInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, copyright) in ctx.debian_copyrights() {
            let Some(header) = copyright.paragraphs().and_then(<[_]>::first) else {
                continue;
            };
            for (old, new) in OBSOLETE {
                if !header.declares(old) {
                    continue;
                }
                ctx.diagnostic(
                    Diagnostic::warn(format!("{old} {new}")).with_location(Location {
                        path: path.clone(),
                        line: header.position(old),
                        column: None,
                    }),
                );
            }
        }
    }
}

const OBSOLETE: [(&str, &str); 5] = [
    ("Format-Specification", "Format"),
    ("Maintainer", "Upstream-Contact"),
    ("Upstream-Maintainer", "Upstream-Contact"),
    ("Contact", "Upstream-Contact"),
    ("Name", "Upstream-Name"),
];

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const FORMAT: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn copyright(header: &str) -> TestFile {
        TestFile::new(format!(
            "{FORMAT}{header}\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![(
                "debian/copyright",
                copyright("Upstream-Name: example\nUpstream-Contact: Someone\n"),
            )],
            // Only the header is checked.
            vec![(
                "debian/copyright",
                TestFile::new(format!(
                    "{FORMAT}\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n\nFiles: a\nName: b\nCopyright: 2024 Someone\nLicense: MIT\n"
                )),
            )],
        ];
        let fail = vec![
            vec![(
                "debian/copyright",
                copyright("Upstream-Maintainer: Someone\n"),
            )],
            vec![(
                "debian/copyright",
                copyright("Name: example\nContact: Someone\n"),
            )],
        ];
        SourceTreeTester::new(ObsoleteFieldInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
