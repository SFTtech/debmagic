use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::pot::{self, Pot};

pub struct NewerDebconfTemplates;

declare_rule! {
    /// `templates.pot` is older than the debconf templates.
    ///
    /// Runs under the same conditions as LN1326. The catalogue that
    /// `intltool-update --pot` generates from the templates is compared with
    /// `debian/po/templates.pot` by `msgcmp --use-untranslated` in both
    /// directions; a difference in either reports the file. Skipped when the
    /// generation itself fails (LN1326). Extra is empty; the Location is
    /// templates.pot. The Rule reports nothing when a tool is not installed.
    NewerDebconfTemplates,
    code = "LN1327",
    tag = "newer-debconf-templates",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NewerDebconfTemplates {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let state = pot::state(ctx);
        if state.pot != Pot::Stale {
            return;
        }
        if let Some(path) = state.templates_pot.clone() {
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
        // templates.pot lacks the `Question.` string that the templates add.
        let stale = tree(vec![(
            "debian/po/templates.pot",
            TestFile::from(
                "msgid \"\"\nmsgstr \"\"\n\n#. Description\n#: ../templates:1001\nmsgid \"Ask\"\nmsgstr \"\"\n",
            ),
        )]);
        let pass = vec![tree(vec![])];
        let fail = vec![stale];
        SourceTreeTester::new(NewerDebconfTemplates, pass.clone(), fail.clone()).test();
        SourcePackageTester::new(
            NewerDebconfTemplates,
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
