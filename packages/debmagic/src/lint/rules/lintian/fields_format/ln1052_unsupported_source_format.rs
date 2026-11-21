use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct UnsupportedSourceFormat;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    UnsupportedSourceFormat,
    code = "LN1052",
    tag = "unsupported-source-format",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UnsupportedSourceFormat {
    fn run(&self, _ctx: &mut SourceContext<'_>) {
        // TODO: port lintian's `fields/format` Check (just lintian-tag unsupported-source-format).
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
        SourceTreeTester::new(UnsupportedSourceFormat, pass, fail).test();
    }
}
