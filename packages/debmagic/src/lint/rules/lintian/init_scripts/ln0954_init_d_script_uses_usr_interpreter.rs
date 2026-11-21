use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct InitDScriptUsesUsrInterpreter;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    InitDScriptUsesUsrInterpreter,
    code = "LN0954",
    tag = "init.d-script-uses-usr-interpreter",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptUsesUsrInterpreter {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `init-d` Check (just lintian-tag init.d-script-uses-usr-interpreter).
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
        BinaryPackageTester::new(InitDScriptUsesUsrInterpreter, pass, fail).test();
    }
}
