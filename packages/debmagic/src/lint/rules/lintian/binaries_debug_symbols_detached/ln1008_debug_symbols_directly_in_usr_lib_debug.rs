use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct DebugSymbolsDirectlyInUsrLibDebug;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    DebugSymbolsDirectlyInUsrLibDebug,
    code = "LN1008",
    tag = "debug-symbols-directly-in-usr-lib-debug",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebugSymbolsDirectlyInUsrLibDebug {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `binaries/debug-symbols/detached` Check (just lintian-tag debug-symbols-directly-in-usr-lib-debug).
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
        BinaryPackageTester::new(DebugSymbolsDirectlyInUsrLibDebug, pass, fail).test();
    }
}
