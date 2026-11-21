use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PrivacyBreachW3cValidHtml;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    PrivacyBreachW3cValidHtml,
    code = "LN1084",
    tag = "privacy-breach-w3c-valid-html",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachW3cValidHtml {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `files/privacy-breach` Check (just lintian-tag privacy-breach-w3c-valid-html).
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
        BinaryPackageTester::new(PrivacyBreachW3cValidHtml, pass, fail).test();
    }
}
