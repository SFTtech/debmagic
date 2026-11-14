use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionContainsDhMakePerlTemplate;

declare_rule! {
    /// A line of the extended description is the `dh-make-perl` boilerplate.
    ///
    /// Reads the Installation control. A line that contains, in any letter
    /// case, `this description was automagically extracted from the module by
    /// dh-make-perl` is reported. Extra is `line N` with N counting from 1 at
    /// the first line after the synopsis (lines made of only ` .` are skipped
    /// but still counted). Location is empty. Udebs are checked too.
    DescriptionContainsDhMakePerlTemplate,
    code = "LN0671",
    tag = "description-contains-dh-make-perl-template",
    default_selected = true,
    experimental = false,
}

const TEMPLATE: &str =
    "this description was automagically extracted from the module by dh-make-perl";

impl BinaryPackageRule for DescriptionContainsDhMakePerlTemplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        for (position, line) in description.numbered_lines() {
            if line.to_lowercase().contains(TEMPLATE) {
                ctx.diagnostic(Diagnostic::warn(format!("line {position}")));
            }
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
            description("synopsis\n A real description."),
            description("synopsis\n This description was automagically extracted from the module"),
            // The synopsis is not an extended line.
            description(
                "This description was automagically extracted from the module by dh-make-perl\n Text.",
            ),
        ];
        let fail = vec![
            description(
                "synopsis\n Text.\n .\n This description was automagically extracted from the module by dh-make-perl",
            ),
            description(
                "synopsis\n THIS DESCRIPTION WAS AUTOMAGICALLY EXTRACTED FROM THE MODULE BY DH-MAKE-PERL.",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(DescriptionContainsDhMakePerlTemplate, pass, fail)
            .test_and_snapshot();
    }
}
