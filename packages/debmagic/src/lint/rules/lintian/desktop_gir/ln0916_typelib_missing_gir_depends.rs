use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct TypelibMissingGirDepends;

declare_rule! {
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    TypelibMissingGirDepends,
    code = "LN0916",
    tag = "typelib-missing-gir-depends",
    default_selected = true,
    experimental = false,
}

impl SourceRule for TypelibMissingGirDepends {
    fn run(&self, _ctx: &mut SourceContext<'_>) {
        // TODO: port lintian's `desktop/gnome/gir` Check (just lintian-tag typelib-missing-gir-depends).
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
        SourceTreeTester::new(TypelibMissingGirDepends, pass, fail).test();
    }
}
