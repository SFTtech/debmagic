use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct ExtendedDescriptionIsEmpty;

declare_rule! {
    /// The `Description` has no extended description.
    ///
    /// Reads the Installation control. The part after the first line must
    /// have no lines at all (a line made of only a space and a dot still
    /// counts as a line). Skipped for udebs, packages without a `Description`
    /// and packages whose name contains `-dbg` (debug packages). Extra and
    /// Location are empty.
    ExtendedDescriptionIsEmpty,
    code = "LN0037",
    tag = "extended-description-is-empty",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ExtendedDescriptionIsEmpty {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() || ctx.package_name().contains("-dbg") {
            return;
        }
        let Some(description) = description(ctx) else {
            return;
        };
        let has_extended_line = !description.lines.is_empty();
        if !has_extended_line {
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
            description("synopsis\n extended"),
            description("synopsis\n extended\n more"),
            description("synopsis\n ."),
            // Debug packages and udebs are exempt.
            description("synopsis").package("example-dbg"),
            description("synopsis").package("example-dbgsym"),
            description("synopsis").udeb(),
        ];
        let fail = vec![
            description("synopsis"),
            description("synopsis "),
            description(""),
            description("synopsis").package("example-debug"),
        ];
        BinaryPackageTester::new(ExtendedDescriptionIsEmpty, pass, fail).test_and_snapshot();
    }
}
