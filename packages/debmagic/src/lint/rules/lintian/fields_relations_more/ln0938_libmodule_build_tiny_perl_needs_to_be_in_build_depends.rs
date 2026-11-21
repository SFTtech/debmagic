use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct LibmoduleBuildTinyPerlNeedsToBeInBuildDepends;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    LibmoduleBuildTinyPerlNeedsToBeInBuildDepends,
    code = "LN0938",
    tag = "libmodule-build-tiny-perl-needs-to-be-in-build-depends",
    default_selected = true,
    experimental = false,
}

impl SourceRule for LibmoduleBuildTinyPerlNeedsToBeInBuildDepends {
    fn run(&self, _ctx: &mut SourceContext<'_>) {
        // TODO: port lintian's `fields/package-relations` Check (just lintian-tag libmodule-build-tiny-perl-needs-to-be-in-build-depends).
        // Report with `ctx.diagnostic(Diagnostic::error(extra).with_location(Location { .. }))`.
        let _ = (Diagnostic::error(""), None::<Location>);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    #[test]
    fn source() {
        // Pass and fail cases; `.test_and_snapshot()` once there are fail cases (`just snap-accept`).
        let pass = Vec::<TestCase>::new();
        let fail = Vec::<TestCase>::new();
        SourceTreeTester::new(LibmoduleBuildTinyPerlNeedsToBeInBuildDepends, pass, fail).test();
    }
}
