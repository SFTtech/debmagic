use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct SynopsisTooLong;

declare_rule! {
    /// The synopsis is longer than 80 characters.
    ///
    /// Not checked when the synopsis is blank. Extra and Location are empty.
    /// Udebs are checked too.
    SynopsisTooLong,
    code = "LN0055",
    tag = "synopsis-too-long",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SynopsisTooLong {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let synopsis = &description.synopsis;
        if !synopsis.trim().is_empty() && synopsis.chars().count() > 80 {
            ctx.diagnostic(Diagnostic::warn(""));
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
            description(&"x ".repeat(40)),
            description("short synopsis"),
        ];
        let fail = vec![
            description(&"x ".repeat(41)),
            description(&format!("a{}", "b".repeat(80))),
        ];
        BinaryPackageTester::new(SynopsisTooLong, pass, fail).test_and_snapshot();
    }
}
