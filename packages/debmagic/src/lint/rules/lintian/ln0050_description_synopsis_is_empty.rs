use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionSynopsisIsEmpty;

declare_rule! {
    /// The first line of the `Description` is empty.
    ///
    /// Reads the Installation control. The synopsis (first line, or the whole
    /// trimmed value when there is one line) must not be only whitespace.
    /// Extra and Location are empty. Udebs are checked too.
    DescriptionSynopsisIsEmpty,
    code = "LN0050",
    tag = "description-synopsis-is-empty",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DescriptionSynopsisIsEmpty {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        if description.synopsis.trim().is_empty() {
            ctx.diagnostic(Diagnostic::error(""));
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
            description("synopsis"),
            description("synopsis\n extended"),
        ];
        let fail = vec![
            description(""),
            description("  "),
            description("\n extended"),
            description("\n extended").udeb(),
        ];
        BinaryPackageTester::new(DescriptionSynopsisIsEmpty, pass, fail).test_and_snapshot();
    }
}
