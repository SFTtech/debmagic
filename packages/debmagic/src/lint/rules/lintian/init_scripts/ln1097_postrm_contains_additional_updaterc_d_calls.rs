use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PostrmContainsAdditionalUpdatercDCalls;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    PostrmContainsAdditionalUpdatercDCalls,
    code = "LN1097",
    tag = "postrm-contains-additional-updaterc.d-calls",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PostrmContainsAdditionalUpdatercDCalls {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `init-d` Check (just lintian-tag postrm-contains-additional-updaterc.d-calls).
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
        BinaryPackageTester::new(PostrmContainsAdditionalUpdatercDCalls, pass, fail).test();
    }
}
