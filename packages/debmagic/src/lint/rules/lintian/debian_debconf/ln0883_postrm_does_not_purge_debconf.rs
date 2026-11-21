use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PostrmDoesNotPurgeDebconf;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    PostrmDoesNotPurgeDebconf,
    code = "LN0883",
    tag = "postrm-does-not-purge-debconf",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PostrmDoesNotPurgeDebconf {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `debian/debconf` Check (just lintian-tag postrm-does-not-purge-debconf).
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
        BinaryPackageTester::new(PostrmDoesNotPurgeDebconf, pass, fail).test();
    }
}
