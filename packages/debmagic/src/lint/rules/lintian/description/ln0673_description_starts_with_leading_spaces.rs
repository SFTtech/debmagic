use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionStartsWithLeadingSpaces;

declare_rule! {
    /// The first line of the extended description is indented by more than the
    /// one mandatory space.
    ///
    /// Reads the Installation control. Only the first line after the synopsis
    /// is looked at, and only when it is not a ` .` separator: it starts with
    /// a space followed by another whitespace character. Extra is `line 1`.
    /// Location is empty. Udebs are checked too.
    DescriptionStartsWithLeadingSpaces,
    code = "LN0673",
    tag = "description-starts-with-leading-spaces",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DescriptionStartsWithLeadingSpaces {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let indented = matches!(
            description.numbered_lines().next(),
            Some((1, line)) if line.strip_prefix(' ').is_some_and(|rest| rest.starts_with(char::is_whitespace))
        );
        if indented {
            ctx.diagnostic(Diagnostic::warn("line 1"));
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
            description("synopsis\n Text.\n   Indented later."),
            // The first line is a separator, so the second is not line 1.
            description("synopsis\n .\n   Indented second."),
            description("   synopsis\n Text."),
        ];
        let fail = vec![
            description("synopsis\n   Oh, and do not start with spaces.\n Text."),
            description("synopsis\n \tTabbed.").udeb(),
        ];
        BinaryPackageTester::new(DescriptionStartsWithLeadingSpaces, pass, fail)
            .test_and_snapshot();
    }
}
