use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct Apache2ModuleDoesNotShipLoadFile;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    Apache2ModuleDoesNotShipLoadFile,
    code = "LN0851",
    tag = "apache2-module-does-not-ship-load-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for Apache2ModuleDoesNotShipLoadFile {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `apache2` Check (just lintian-tag apache2-module-does-not-ship-load-file).
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
        BinaryPackageTester::new(Apache2ModuleDoesNotShipLoadFile, pass, fail).test();
    }
}
