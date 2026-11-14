use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionStartsWithPackageName;

declare_rule! {
    /// The synopsis starts with the package name.
    ///
    /// Case-insensitive match of the package name at the start of the
    /// synopsis, followed by a word boundary. Not checked when the synopsis is
    /// blank. Extra and Location are empty. Udebs are checked too.
    DescriptionStartsWithPackageName,
    code = "LN0051",
    tag = "description-starts-with-package-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DescriptionStartsWithPackageName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let synopsis = &description.synopsis;
        if synopsis.trim().is_empty() {
            return;
        }
        let pattern = format!(r"(?i)^{}\b", regex::escape(ctx.package_name()));
        if Regex::new(&pattern).is_ok_and(|regex| regex.is_match(synopsis)) {
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
            description("a tool for examples"),
            // No word boundary after the name.
            description("examples of things"),
            description("the example tool"),
        ];
        let fail = vec![
            description("example tool"),
            description("Example is a tool"),
            description("EXAMPLE: tool"),
            description("example").udeb(),
        ];
        BinaryPackageTester::new(DescriptionStartsWithPackageName, pass, fail).test_and_snapshot();
    }
}
