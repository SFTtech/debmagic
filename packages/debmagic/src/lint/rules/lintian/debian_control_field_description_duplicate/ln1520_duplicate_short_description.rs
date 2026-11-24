use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::ln1519_duplicate_long_description::duplicate_names;

pub struct DuplicateShortDescription;

declare_rule! {
    /// Two or more installables of `debian/control` (udebs not counted) have
    /// the same short description: the first line of `Description`, trimmed.
    /// The extra is the sorted names of the packages, separated by spaces, and
    /// the Location is `debian/control`.
    ///
    /// A synopsis that holds a `${...}` substitution variable is not compared,
    /// as lintian assumes substvars are handled. Info severity and not selected
    /// by default. The extended description is not looked at here (see
    /// `LN1519`).
    DuplicateShortDescription,
    code = "LN1520",
    tag = "duplicate-short-description",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DuplicateShortDescription {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        for names in duplicate_names(control, synopsis_part) {
            ctx.diagnostic(
                Diagnostic::warn(names.join(" "))
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: PathBuf::from("debian/control"),
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// The short description: the first line of `Description`.
fn synopsis_part(description: &str) -> &str {
    description
        .split_once('\n')
        .map_or(description, |(synopsis, _)| synopsis)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    const HEAD: &str = "Source: example\nMaintainer: A <a@example.com>\n\n";

    fn control(paragraphs: &str) -> TestCase {
        TestCase::from([("debian/control", format!("{HEAD}{paragraphs}"))])
    }

    #[test]
    fn source() {
        let pass = vec![
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: one\n\nPackage: pkg-b\nArchitecture: all\nDescription: two\n",
            ),
            // The same extended text is not a short description.
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: one\n long\n\nPackage: pkg-b\nArchitecture: all\nDescription: two\n long\n",
            ),
            // A udeb does not count.
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: same\n\nPackage: pkg-udeb\nPackage-Type: udeb\nArchitecture: all\nDescription: same\n",
            ),
            // Substitution variables are not compared.
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: ${x}\n\nPackage: pkg-b\nArchitecture: all\nDescription: ${x}\n",
            ),
        ];
        let fail = vec![
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: same\n long\n\nPackage: pkg-b\nArchitecture: all\nDescription: same\n other\n",
            ),
            // Trimmed at both ends.
            control(
                "Package: pkg-a\nArchitecture: all\nDescription:   same  \n\nPackage: pkg-b\nArchitecture: all\nDescription: same\n\nPackage: pkg-c\nArchitecture: all\nDescription: same\n",
            ),
        ];
        SourceTreeTester::new(DuplicateShortDescription, pass, fail).test_and_snapshot();
    }
}
