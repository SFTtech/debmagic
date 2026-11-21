use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct UpstartJobInEtcInitDNotRegisteredViaUpdateRcD;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    UpstartJobInEtcInitDNotRegisteredViaUpdateRcD,
    code = "LN1101",
    tag = "upstart-job-in-etc-init.d-not-registered-via-update-rc.d",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UpstartJobInEtcInitDNotRegisteredViaUpdateRcD {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `init-d` Check (just lintian-tag upstart-job-in-etc-init.d-not-registered-via-update-rc.d).
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
        BinaryPackageTester::new(UpstartJobInEtcInitDNotRegisteredViaUpdateRcD, pass, fail).test();
    }
}
