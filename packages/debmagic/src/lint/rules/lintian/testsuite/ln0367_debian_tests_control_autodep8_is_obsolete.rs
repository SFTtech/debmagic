use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct DebianTestsControlAutodep8IsObsolete;

declare_rule! {
    /// `debian/tests/control.autodep8` is still shipped.
    ///
    /// Any entry at `debian/tests/control.autodep8` is reported, whatever its
    /// type. No extra. Location is that path, without a line.
    DebianTestsControlAutodep8IsObsolete,
    code = "LN0367",
    tag = "debian-tests-control-autodep8-is-obsolete",
    default_selected = true,
    experimental = false,
}

const AUTODEP8: &str = "debian/tests/control.autodep8";

impl SourceRule for DebianTestsControlAutodep8IsObsolete {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let present = ctx
            .files()
            .is_some_and(|files| files.get(AUTODEP8).is_some());
        if !present {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
            path: PathBuf::from(AUTODEP8),
            line: None,
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            vec![(
                "debian/tests/control",
                TestFile::new("Tests: smoke\nDepends: @\n"),
            )],
            vec![
                (
                    "debian/tests/control",
                    TestFile::new("Tests: smoke\nDepends: @\n"),
                ),
                ("debian/tests/control.autodep8.bak", TestFile::new("old\n")),
            ],
            vec![("debian/tests/autodep8", TestFile::new("old\n"))],
        ];
        let fail = vec![
            vec![(
                "debian/tests/control.autodep8",
                TestFile::new("Tests: smoke\n"),
            )],
            vec![
                (
                    "debian/tests/control",
                    TestFile::new("Tests: smoke\nDepends: @\n"),
                ),
                (
                    "debian/tests/control.autodep8",
                    TestFile::new("Tests: smoke\n"),
                ),
            ],
        ];
        SourceTreeTester::new(DebianTestsControlAutodep8IsObsolete, pass, fail).test_and_snapshot();
    }
}
