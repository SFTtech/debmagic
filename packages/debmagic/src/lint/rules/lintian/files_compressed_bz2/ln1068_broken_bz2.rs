use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct BrokenBz2;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    BrokenBz2,
    code = "LN1068",
    tag = "broken-bz2",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BrokenBz2 {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `files/compressed/bz2` Check (just lintian-tag broken-bz2).
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
        BinaryPackageTester::new(BrokenBz2, pass, fail).test();
    }
}
