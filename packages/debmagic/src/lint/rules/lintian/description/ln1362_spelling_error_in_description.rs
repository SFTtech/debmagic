use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::description_field::description;
use crate::lint::spelling::{Exceptions, check_spelling};

pub struct SpellingErrorInDescription;

declare_rule! {
    /// The extended description has a misspelling, a repeated word, or a
    /// misspelled phrase from lintian's spelling lists.
    ///
    /// Checks the extended description only (lines after the synopsis, joined
    /// with newlines), as lintian's `check_spelling` does: brackets are
    /// dropped, parentheses are dropped except `(s)`, and a hyphen broken at a
    /// line end is joined. Each misspelled word is reported once, with extra
    /// `<word> <correction>` (capitalization follows the word). A repeated
    /// word is reported once with extra `<word> <word> (duplicate word)
    /// <word>`, unless it is quoted, ends a sentence, or is written as
    /// `word (word` or `word) word`. Multi-word phrases are reported once
    /// each, with extra `"<phrase>" "<correction>"`. Location is empty.
    /// Udebs are checked too.
    /// Difference from lintian: the spelling exceptions of the lintian group
    /// are not applied, as a Description has no group. The synopsis is
    /// `spelling-error-in-description-synopsis`, not checked here.
    SpellingErrorInDescription,
    code = "LN1362",
    tag = "spelling-error-in-description",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for SpellingErrorInDescription {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let extended = description.lines.join("\n");
        for (found, correction) in check_spelling(&extended, &Exceptions::new(), true) {
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

    fn description(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Description", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            description("synopsis\n Tools for the job."),
            // The synopsis is not part of this Tag.
            description("mroe tools\n A tool."),
            // Quoted repeats and acronyms are not reported.
            description("synopsis\n Says \"that that\" thing."),
            description("synopsis\n The USA tool."),
        ];
        let fail = vec![
            description("synopsis\n A tool with mroe features."),
            description("synopsis\n Several these these words."),
            description("synopsis\n Permits to use it.\n This packages work."),
            description("synopsis\n Mroe tools.").udeb(),
        ];
        BinaryPackageTester::new(SpellingErrorInDescription, pass, fail).test_and_snapshot();
    }
}
