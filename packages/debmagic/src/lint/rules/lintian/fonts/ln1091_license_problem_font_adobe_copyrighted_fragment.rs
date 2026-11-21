use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct LicenseProblemFontAdobeCopyrightedFragment;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    LicenseProblemFontAdobeCopyrightedFragment,
    code = "LN1091",
    tag = "license-problem-font-adobe-copyrighted-fragment",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LicenseProblemFontAdobeCopyrightedFragment {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `fonts/postscript/type1` Check (just lintian-tag license-problem-font-adobe-copyrighted-fragment).
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
        BinaryPackageTester::new(LicenseProblemFontAdobeCopyrightedFragment, pass, fail).test();
    }
}
