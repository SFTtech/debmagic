use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct FontInNonFontPackage;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    FontInNonFontPackage,
    code = "LN1089",
    tag = "font-in-non-font-package",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for FontInNonFontPackage {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `fonts` Check (just lintian-tag font-in-non-font-package).
        // Report with `ctx.diagnostic(Diagnostic::warn(extra).with_location(Location { .. }))`.
        let _ = (Diagnostic::warn(""), None::<Location>);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester};

    use super::*;

    #[test]
    fn binary() {
        // Pass and fail cases; `.test_and_snapshot()` once there are fail cases (`just snap-accept`).
        let pass = Vec::<BinaryPackageCase>::new();
        let fail = Vec::<BinaryPackageCase>::new();
        BinaryPackageTester::new(FontInNonFontPackage, pass, fail).test();
    }
}
