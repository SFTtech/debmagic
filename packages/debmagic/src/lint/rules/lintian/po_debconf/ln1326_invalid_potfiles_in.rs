use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::pot::{self, Pot};

pub struct InvalidPotfilesIn;

declare_rule! {
    /// `debian/po/POTFILES.in` cannot be turned into a template catalogue.
    ///
    /// Runs when the package uses po-debconf (translatable templates and a PO
    /// directory) and POTFILES.in, every file it lists under `debian/`, and
    /// `templates.pot` are present. `intltool-update --pot` is then run in a
    /// copy of the packaging with `srcdir` set to the PO directory; a non-zero
    /// exit reports the file. Skipped when a listed file is missing (LN0904)
    /// or POTFILES.in is missing (LN0905). Extra is empty; the Location is
    /// POTFILES.in. The Rule reports nothing when `intltool-update` or
    /// `msgcmp` is not installed. Unlike lintian, an error in this step does
    /// not stop the other po-debconf Rules.
    InvalidPotfilesIn,
    code = "LN1326",
    tag = "invalid-potfiles-in",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidPotfilesIn {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let state = pot::state(ctx);
        if state.pot != Pot::Invalid {
            return;
        }
        if let Some(path) = state.potfiles_in.clone() {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::super::pot::test_support::tree;
    use super::*;

    #[test]
    fn test() {
        // A non-ASCII string marked for translation makes xgettext fail when
        // the file has no charset, as the Check's fixture does.
        let invalid = tree(vec![(
            "debian/templates",
            TestFile::from("Template: example/foo\nType: string\n_Description: Caf\u{e9}\n"),
        )]);
        let pass = vec![
            tree(vec![]),
            tree(vec![("debian/po/POTFILES.in", TestFile::from(""))]),
        ];
        let fail = vec![invalid];
        SourceTreeTester::new(InvalidPotfilesIn, pass.clone(), fail.clone()).test();
        SourcePackageTester::new(
            InvalidPotfilesIn,
            pass.into_iter()
                .map(SourcePackageCase::from)
                .collect::<Vec<_>>(),
            fail.into_iter()
                .map(SourcePackageCase::from)
                .collect::<Vec<_>>(),
        )
        .test();
    }
}
