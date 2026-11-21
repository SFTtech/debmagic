use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ControlInterpreterWithoutPredepends;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    ControlInterpreterWithoutPredepends,
    code = "LN0980",
    tag = "control-interpreter-without-predepends",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ControlInterpreterWithoutPredepends {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `scripts` Check (just lintian-tag control-interpreter-without-predepends).
        // Report with `ctx.diagnostic(Diagnostic::error(extra).with_location(Location { .. }))`.
        let _ = (Diagnostic::error(""), None::<Location>);
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
        BinaryPackageTester::new(ControlInterpreterWithoutPredepends, pass, fail).test();
    }
}
