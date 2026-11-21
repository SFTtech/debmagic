use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct SystemdServiceInOddLocation;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    SystemdServiceInOddLocation,
    code = "LN0991",
    tag = "systemd-service-in-odd-location",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SystemdServiceInOddLocation {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `systemd` Check (just lintian-tag systemd-service-in-odd-location).
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
        BinaryPackageTester::new(SystemdServiceInOddLocation, pass, fail).test();
    }
}
