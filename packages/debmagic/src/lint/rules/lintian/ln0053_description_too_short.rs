use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionTooShort;

declare_rule! {
    /// The synopsis is a single word.
    ///
    /// The synopsis (not blank) contains no whitespace. Extra is the synopsis.
    /// Location is empty. Udebs are checked too.
    DescriptionTooShort,
    code = "LN0053",
    tag = "description-too-short",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DescriptionTooShort {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let synopsis = &description.synopsis;
        if synopsis.trim().is_empty() {
            return;
        }
        if !synopsis.contains(char::is_whitespace) {
            ctx.diagnostic(Diagnostic::error(synopsis.clone()));
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
            description("two words"),
            description(""),
        ];
        let fail = vec![
            description("Short"),
            description("Short\n extended text"),
            description("Short").udeb(),
        ];
        BinaryPackageTester::new(DescriptionTooShort, pass, fail).test_and_snapshot();
    }
}
