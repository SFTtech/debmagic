use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::description_field::description;

pub struct OddMarkInDescription;

declare_rule! {
    /// A comma is followed by neither whitespace nor a digit, as in
    /// `300,000`. The synopsis and each extended description line are checked.
    ///
    /// Lintian's `fields/description` Check. Only the comma case is
    /// implemented; lintian reports no other odd marks. Extra is `comma not
    /// followed by whitespace (synopsis)` for the synopsis, or `comma not
    /// followed by whitespace (line N)` for an extended line, where N counts
    /// from 1 at the first line after the synopsis (blank ` .` separators
    /// count). Location is empty. Severity is pedantic and the Tag is not
    /// selected by default, as in lintian. Udebs are checked too.
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
        if COMMA_WITHOUT_SPACE.is_match(&description.synopsis) {
            ctx.diagnostic(
                Diagnostic::warn("comma not followed by whitespace (synopsis)")
                    .with_severity(Severity::Pedantic),
            );
        }
        for (position, line) in description.numbered_lines() {
            if COMMA_WITHOUT_SPACE.is_match(line) {
                ctx.diagnostic(
                    Diagnostic::warn(format!(
                        "comma not followed by whitespace (line {position})"
                    ))
                    .with_severity(Severity::Pedantic),
                );
            }
        }
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
            // A trailing comma has nothing after it.
            synopsis("tools,\n A tool."),
            // A comma before a digit or whitespace in an extended line is fine.
            synopsis("tools\n A tool, 300,000 and, more.\n .\n Done, ok."),
            synopsis("tools\n A tool,\n Trailing comma."),
        ];
        let fail = vec![
            synopsis("tools,and more\n A tool."),
            synopsis("tools,a\n A tool."),
            synopsis("counts 300,x\n A tool.").udeb(),
            // Extended lines are checked too; positions count blank separators.
            synopsis("tools\n A tool, that is,it works."),
            synopsis("tools\n A tool.\n .\n Uses {heap,stack} memory."),
        ];
        BinaryPackageTester::new(OddMarkInDescription, pass, fail).test_and_snapshot();
    }
}
