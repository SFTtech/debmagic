use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct UnknownPriority;

declare_rule! {
    /// The `Priority` field of the Installation control is not a Policy priority.
    ///
    /// The known priorities are the list in lintian's `fields/priorities`:
    /// `required`, `important`, `standard`, `optional`. Extra is the unfolded
    /// field value. A declared but empty `Priority` reports an empty extra.
    /// An undeclared `Priority` is skipped.
    ///
    /// `Priority: extra` is remapped to `optional` before the test on
    /// packages without `Auto-Built-Package`, so it does not report there. On
    /// an auto-generated package `extra` stays and reports. Udebs are checked
    /// too. The Source paragraph is not read.
    UnknownPriority,
    code = "LN1062",
    tag = "unknown-priority",
    default_selected = true,
    experimental = false,
}

/// `data/fields/priorities` (lintian 2.144.0).
const PRIORITIES: &str = include_str!("../../../data/fields/priorities");

impl BinaryPackageRule for UnknownPriority {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        if !fields.declares("Priority") {
            return;
        }
        let mut priority = fields.unfolded_value("Priority").unwrap_or_default();
        if priority == "extra" && !fields.declares("Auto-Built-Package") {
            priority = "optional".to_string();
        }
        if crate::lint::data::lines(PRIORITIES).any(|known| known == priority) {
            return;
        }
        ctx.diagnostic(Diagnostic::error(priority));
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
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            priority("required"),
            priority("important"),
            priority("standard"),
            priority("optional"),
            priority("extra"),
            priority("optional").udeb(),
        ];
        let fail = vec![
            priority("is weird"),
            // Folded value: the unfolded text is what is tested.
            priority("opt\n ional"),
            priority("Optional"),
            priority(""),
            priority("nonsense").udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Priority", "extra")
                .field("Auto-Built-Package", "debug-symbols"),
        ];
        BinaryPackageTester::new(UnknownPriority, pass, fail).test_and_snapshot();
    }
}
