use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct SystemdServiceFileUsesDeprecatedSyslogFacility;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    SystemdServiceFileUsesDeprecatedSyslogFacility,
    code = "LN0990",
    tag = "systemd-service-file-uses-deprecated-syslog-facility",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SystemdServiceFileUsesDeprecatedSyslogFacility {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `systemd` Check (just lintian-tag systemd-service-file-uses-deprecated-syslog-facility).
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
        BinaryPackageTester::new(SystemdServiceFileUsesDeprecatedSyslogFacility, pass, fail).test();
    }
}
