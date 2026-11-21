use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct NonStandardFilePermissionsForEtcInitDScript;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    NonStandardFilePermissionsForEtcInitDScript,
    code = "LN1078",
    tag = "non-standard-file-permissions-for-etc-init.d-script",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NonStandardFilePermissionsForEtcInitDScript {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `files/init` Check (just lintian-tag non-standard-file-permissions-for-etc-init.d-script).
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
        BinaryPackageTester::new(NonStandardFilePermissionsForEtcInitDScript, pass, fail).test();
    }
}
