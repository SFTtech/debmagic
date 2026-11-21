use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct DebconfConfigNotExecutable;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    DebconfConfigNotExecutable,
    code = "LN0886",
    tag = "debconf-config-not-executable",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebconfConfigNotExecutable {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `debian/debconf` Check (just lintian-tag debconf-config-not-executable).
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
        BinaryPackageTester::new(DebconfConfigNotExecutable, pass, fail).test();
    }
}
