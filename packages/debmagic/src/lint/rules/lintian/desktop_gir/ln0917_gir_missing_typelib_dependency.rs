use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct GirMissingTypelibDependency;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    GirMissingTypelibDependency,
    code = "LN0917",
    tag = "gir-missing-typelib-dependency",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for GirMissingTypelibDependency {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `desktop/gnome/gir` Check (just lintian-tag gir-missing-typelib-dependency).
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
        BinaryPackageTester::new(GirMissingTypelibDependency, pass, fail).test();
    }
}
