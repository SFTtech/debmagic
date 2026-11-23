use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::description_field::description;

pub struct OddMarkInDescription;

declare_rule! {
    /// The synopsis has a comma that is followed by neither whitespace nor a
    /// digit, as in `300,000`.
    ///
    /// Lintian's `fields/description` Check, on the synopsis (the first line of
    /// `Description`, as Check splits it). Only the comma case is implemented;
    /// lintian reports no other odd marks. Extra is `comma not followed by
    /// whitespace (synopsis)`. Location is empty. Severity is pedantic and the
    /// Tag is not selected by default, as in lintian. Udebs are checked too.
    OddMarkInDescription,
    code = "LN1668",
    tag = "odd-mark-in-description",
    default_selected = false,
    experimental = false,
}

static COMMA_WITHOUT_SPACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r",[^\s\d]").expect("valid regex"));

impl BinaryPackageRule for OddMarkInDescription {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        if !COMMA_WITHOUT_SPACE.is_match(&description.synopsis) {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn("comma not followed by whitespace (synopsis)")
                .with_severity(Severity::Pedantic),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn synopsis(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Description", value)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            synopsis("tools, and more\n A tool."),
            synopsis("counts 300, 000\n A tool."),
            synopsis("counts 300,000 items\n A tool."),
            // Only the synopsis is checked.
            synopsis("tools\n A tool, that is,it works."),
            // A trailing comma has nothing after it.
            synopsis("tools,\n A tool."),
        ];
        let fail = vec![
            synopsis("tools,and more\n A tool."),
            synopsis("tools,a\n A tool."),
            synopsis("counts 300,x\n A tool.").udeb(),
        ];
        BinaryPackageTester::new(OddMarkInDescription, pass, fail).test_and_snapshot();
    }
}
