use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsBrokenSymlinkWildcard;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    PackageContainsBrokenSymlinkWildcard,
    code = "LN0929",
    tag = "package-contains-broken-symlink-wildcard",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsBrokenSymlinkWildcard {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `files/symbolic-links/broken` Check (just lintian-tag package-contains-broken-symlink-wildcard).
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
        BinaryPackageTester::new(PackageContainsBrokenSymlinkWildcard, pass, fail).test();
    }
}
