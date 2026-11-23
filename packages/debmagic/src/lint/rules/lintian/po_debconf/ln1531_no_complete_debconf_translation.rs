use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::pot::{self, Pot};

pub struct NoCompleteDebconfTranslation;

declare_rule! {
    /// No PO translation of the debconf templates is complete.
    ///
    /// Runs when the package uses po-debconf and the `intltool-update` step
    /// (LN1326) did not fail. Every `.po` file in the PO directory with a
    /// `msgstr` line is checked with `msgfmt -o /dev/null --statistics`; a
    /// file whose statistics read `N translated messages.` is complete. Extra
    /// is empty and there is no Location. Severity info, not selected by
    /// default. The Rule reports nothing when `msgfmt` is not installed.
    NoCompleteDebconfTranslation,
    code = "LN1531",
    tag = "no-complete-debconf-translation",
    default_selected = false,
    experimental = false,
}

impl SourceRule for NoCompleteDebconfTranslation {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let state = pot::state(ctx);
        if !state.using_po || state.pot == Pot::Invalid {
            return;
        }
        if state.full_translation == Some(false) {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::super::pot::test_support::tree;
    use super::*;

    const COMPLETE_FR: &str = "msgid \"\"\nmsgstr \"\"\n\"Content-Type: text/plain; charset=UTF-8\\n\"\n\nmsgid \"Ask\"\nmsgstr \"Demander\"\n\nmsgid \"Question.\"\nmsgstr \"Question.\"\n";
    const PARTIAL_FR: &str = "msgid \"\"\nmsgstr \"\"\n\"Content-Type: text/plain; charset=UTF-8\\n\"\n\nmsgid \"Ask\"\nmsgstr \"Demander\"\n\nmsgid \"Question.\"\nmsgstr \"\"\n";

    #[test]
    fn test() {
        let complete = tree(vec![("debian/po/fr.po", TestFile::from(COMPLETE_FR))]);
        let partial = tree(vec![("debian/po/fr.po", TestFile::from(PARTIAL_FR))]);
        let pass = vec![complete];
        let fail = vec![tree(vec![]), partial];
        SourceTreeTester::new(NoCompleteDebconfTranslation, pass.clone(), fail.clone()).test();
        SourcePackageTester::new(
            NoCompleteDebconfTranslation,
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
