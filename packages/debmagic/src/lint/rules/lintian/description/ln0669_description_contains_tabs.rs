use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionContainsTabs;

declare_rule! {
    /// The `Description` contains a tab character.
    ///
    /// Reads the Installation control. Reported at most once per package: for
    /// the synopsis (when it is not blank) with an empty extra, otherwise for
    /// the first extended line with a tab, with extra `line N` counting from 1
    /// at the first line after the synopsis (lines made of only ` .` are
    /// skipped but still counted). Location is empty. Udebs are checked too.
    DescriptionContainsTabs,
    code = "LN0669",
    tag = "description-contains-tabs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DescriptionContainsTabs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        if !description.synopsis.trim().is_empty() && description.synopsis.contains('\t') {
            ctx.diagnostic(Diagnostic::error(""));
            return;
        }
        if let Some((position, _)) = description
            .numbered_lines()
            .find(|(_, line)| line.contains('\t'))
        {
            ctx.diagnostic(Diagnostic::error(format!("line {position}")));
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
            description("synopsis\n A line.\n Another."),
            description("synopsis\n A line\\twith a literal backslash."),
        ];
        let fail = vec![
            description("syn\topsis\n A line."),
            description("synopsis\n A line.\n .\n With\ta tab.\n\tAnd another."),
            description("syn\topsis\n With\ta tab too.").udeb(),
        ];
        BinaryPackageTester::new(DescriptionContainsTabs, pass, fail).test_and_snapshot();
    }
}
