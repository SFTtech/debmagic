use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct PriorityExtraIsReplacedByPriorityOptional;

declare_rule! {
    /// Installation control `Priority` is `extra`, which Policy 4.0.1 deprecated.
    ///
    /// Skipped for an auto-generated package, one whose control declares
    /// `Auto-Built-Package` (such as `-dbgsym`). Only the exact value `extra`
    /// matches; no other priority is checked here (`unknown-priority` is
    /// LN1062). Binary only: lintian's source paragraph check for `extra` is
    /// not ported, since this Rule's Subject is the binary package. Udebs are
    /// checked too. Extra is empty.
    PriorityExtraIsReplacedByPriorityOptional,
    code = "LN1061",
    tag = "priority-extra-is-replaced-by-priority-optional",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PriorityExtraIsReplacedByPriorityOptional {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        if !fields.declares("Priority") || fields.declares("Auto-Built-Package") {
            return;
        }
        if fields.unfolded_value("Priority").as_deref() == Some("extra") {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn priority(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Priority", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            priority("optional"),
            priority("required"),
            priority("Extra"),
            priority("optional").udeb(),
            priority("extra")
                .field("Auto-Built-Package", "debug-symbols")
                .package("foo-dbgsym"),
        ];
        let fail = vec![
            priority("extra"),
            // Surrounding space is trimmed, as unfolded_value does.
            priority(" extra ").udeb(),
            priority("extra").package("foo-dbgsym"),
        ];
        BinaryPackageTester::new(PriorityExtraIsReplacedByPriorityOptional, pass, fail)
            .test_and_snapshot();
    }
}
