use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionContainsInvalidControlStatement;

declare_rule! {
    /// A line of the extended description looks like a control statement but
    /// is not a bare ` .`.
    ///
    /// Reads the Installation control. A line is a bad control statement when
    /// it is a space, a dot and then other text, or a space, more whitespace,
    /// a dot and nothing else. Lines made of only ` .` are the valid
    /// separators. Extra is `line N` with N counting from 1 at the first line
    /// after the synopsis. Location is empty. Udebs are checked too.
    DescriptionContainsInvalidControlStatement,
    code = "LN0670",
    tag = "description-contains-invalid-control-statement",
    default_selected = true,
    experimental = false,
}

static DOT_WITH_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ \.\s*\S").expect("valid regex"));
static INDENTED_DOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ \s+\.\s*$").expect("valid regex"));

impl BinaryPackageRule for DescriptionContainsInvalidControlStatement {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        for (position, line) in description.numbered_lines() {
            if DOT_WITH_TEXT.is_match(line) || INDENTED_DOT.is_match(line) {
                ctx.diagnostic(Diagnostic::error(format!("line {position}")));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn description(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Description", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            description("synopsis\n A line.\n .\n Another."),
            description("synopsis\n A line.\n . \n Another."),
            description("synopsis\n An ellipsis ... in text."),
            description(".control in the synopsis\n A line."),
        ];
        let fail = vec![
            description("synopsis\n A line.\n .control statements are not allowed."),
            description("synopsis\n A line.\n   .\n Another."),
            description("synopsis\n .\n .a\n .\n .b").udeb(),
        ];
        BinaryPackageTester::new(DescriptionContainsInvalidControlStatement, pass, fail)
            .test_and_snapshot();
    }
}
