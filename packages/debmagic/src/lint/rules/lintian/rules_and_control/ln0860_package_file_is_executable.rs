use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct PackageFileIsExecutable;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    PackageFileIsExecutable,
    code = "LN0860",
    tag = "package-file-is-executable",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PackageFileIsExecutable {
    fn run(&self, _ctx: &mut SourceContext<'_>) {
        // TODO: port lintian's `debhelper` Check (just lintian-tag package-file-is-executable).
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
        SourceTreeTester::new(PackageFileIsExecutable, pass, fail).test();
    }
}
