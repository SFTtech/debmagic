use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::description_field::description;
use crate::lint::spelling::check_spelling_picky;

pub struct CapitalizationErrorInDescription;

declare_rule! {
    /// The extended description has a word that lintian's picky list of
    /// capitalization errors corrects, such as `debian` for `Debian`.
    ///
    /// Checks the extended description only (lines after the synopsis, joined
    /// with spaces), as lintian's `check_spelling_picky` does. Text in square
    /// brackets is skipped. The text `meta package` is reported as
    /// `metapackage`. Each misspelled word is reported once, with extra
    /// `<word> <correction>`. Location is empty. Udebs are checked too.
    /// Difference from lintian: the synopsis is not checked here (that is
    /// `capitalization-error-in-description-synopsis`) and the spelling
    /// exceptions of lintian's group are not applied.
    CapitalizationErrorInDescription,
    code = "LN1359",
    tag = "capitalization-error-in-description",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for CapitalizationErrorInDescription {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let extended = description.lines.join("\n");
        for (word, correction) in check_spelling_picky(&extended) {
            ctx.diagnostic(
                Diagnostic::warn(format!("{word} {correction}")).with_severity(Severity::Info),
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
            description("synopsis\n Written by the upstream team."),
            // The synopsis is not part of this Tag.
            description("Debian tools\n A set of tools."),
            // Bracketed text is not checked.
            description("synopsis\n [debian] tools."),
        ];
        let fail = vec![
            description("synopsis\n A tool for debian systems."),
            description("synopsis\n Used in a meta package, and debian."),
            description("synopsis\n debian and debian again.").udeb(),
        ];
        BinaryPackageTester::new(CapitalizationErrorInDescription, pass, fail).test_and_snapshot();
    }
}
