use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, SourceRule};

pub struct SyntaxErrorInDebconfTemplate;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    SyntaxErrorInDebconfTemplate,
    code = "LN0884",
    tag = "syntax-error-in-debconf-template",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SyntaxErrorInDebconfTemplate {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {
        // TODO: port lintian's `debian/debconf` Check (just lintian-tag syntax-error-in-debconf-template).
        // Report with `ctx.diagnostic(Diagnostic::error(extra).with_location(Location { .. }))`.
        let _ = (Diagnostic::error(""), None::<Location>);
    }
}

impl SourceRule for SyntaxErrorInDebconfTemplate {
    fn run(&self, _ctx: &mut SourceContext<'_>) {
        // TODO: port lintian's `debian/debconf` Check (just lintian-tag syntax-error-in-debconf-template).
        // Report with `ctx.diagnostic(Diagnostic::error(extra).with_location(Location { .. }))`.
        let _ = (Diagnostic::error(""), None::<Location>);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestCase};

    use super::*;

    #[test]
    fn binary() {
        // Pass and fail cases; `.test_and_snapshot()` once there are fail cases (`just snap-accept`).
        let pass = Vec::<BinaryPackageCase>::new();
        let fail = Vec::<BinaryPackageCase>::new();
        BinaryPackageTester::new(SyntaxErrorInDebconfTemplate, pass, fail).test();
    }

    #[test]
    fn source() {
        // Pass and fail cases; `.test_and_snapshot()` once there are fail cases (`just snap-accept`).
        let pass = Vec::<TestCase>::new();
        let fail = Vec::<TestCase>::new();
        SourceTreeTester::new(SyntaxErrorInDebconfTemplate, pass, fail).test();
    }
}
