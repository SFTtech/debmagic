use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct MissingFileFromPotfilesIn;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    MissingFileFromPotfilesIn,
    code = "LN0904",
    tag = "missing-file-from-potfiles-in",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MissingFileFromPotfilesIn {
    fn run(&self, _ctx: &mut SourceContext<'_>) {
        // TODO: port lintian's `debian/po-debconf` Check (just lintian-tag missing-file-from-potfiles-in).
        // Report with `ctx.diagnostic(Diagnostic::warn(extra).with_location(Location { .. }))`.
        let _ = (Diagnostic::warn(""), None::<Location>);
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
        SourceTreeTester::new(MissingFileFromPotfilesIn, pass, fail).test();
    }
}
