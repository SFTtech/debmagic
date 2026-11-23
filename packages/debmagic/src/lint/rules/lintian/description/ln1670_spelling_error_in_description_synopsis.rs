use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::spelling::{check_spelling, group_exceptions};

use super::description_field::description;

pub struct SpellingErrorInDescriptionSynopsis;

declare_rule! {
    /// The synopsis (first line of `Description`) has a misspelling from
    /// lintian's spelling lists.
    ///
    /// Lintian's `fields/description` Check runs `check_spelling` on the
    /// synopsis with the group's spelling exceptions, without the duplicate
    /// word check. Each misspelled word is reported once with extra
    /// `<word> <correction>`, capitalization following the word. Location is
    /// empty. Udebs are checked too. The capitalization of the synopsis is
    /// `capitalization-error-in-description-synopsis`, not this Tag.
    SpellingErrorInDescriptionSynopsis,
    code = "LN1670",
    tag = "spelling-error-in-description-synopsis",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for SpellingErrorInDescriptionSynopsis {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        if description.synopsis.is_empty() {
            return;
        }
        let exceptions = group_exceptions(ctx);
        for (found, correction) in check_spelling(&description.synopsis, &exceptions, false) {
            ctx.diagnostic(
                Diagnostic::warn(format!("{found} {correction}")).with_severity(Severity::Info),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn synopsis(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Description", value)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            synopsis("development tools\n Tools for the job."),
            // The extended description is not part of this Tag.
            synopsis("tools\n A developement tool."),
            // A repeated word is not reported in the synopsis.
            synopsis("tools tools\n A tool."),
        ];
        let fail = vec![
            synopsis("developement development\n A tool."),
            synopsis("developement tools\n A tool.").udeb(),
        ];
        BinaryPackageTester::new(SpellingErrorInDescriptionSynopsis, pass, fail)
            .test_and_snapshot();
    }
}
