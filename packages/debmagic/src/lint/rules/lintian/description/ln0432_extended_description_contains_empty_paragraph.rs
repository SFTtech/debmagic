use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct ExtendedDescriptionContainsEmptyParagraph;

declare_rule! {
    /// The extended description contains an empty paragraph.
    ///
    /// Installation control. The extended text is everything after the first
    /// newline of `Description`. An empty paragraph is a leading ` .` line, two
    /// ` .` lines in a row, or a trailing ` .` line. Udebs are not checked.
    /// Extra is empty.
    ExtendedDescriptionContainsEmptyParagraph,
    code = "LN0432",
    tag = "extended-description-contains-empty-paragraph",
    default_selected = true,
    experimental = false,
}

static EMPTY_PARAGRAPH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A \.\s*\n|\n \.\s*\n \.\s*\n|\n \.\s*\n?\z").expect("valid regex")
});

impl BinaryPackageRule for ExtendedDescriptionContainsEmptyParagraph {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(value) = ctx
            .installation_control()
            .fields()
            .untrimmed_value("Description")
        else {
            return;
        };
        let Some((_, extended)) = value.split_once('\n') else {
            return;
        };
        if EMPTY_PARAGRAPH.is_match(extended) {
            ctx.diagnostic(Diagnostic::warn(""));
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
            description("synopsis"),
            description("synopsis\n A paragraph.\n .\n Another.\n"),
            description("synopsis\n .\n Empty first.").udeb(),
        ];
        let fail = vec![
            description("synopsis\n .\n The first paragraph\n is empty.\n"),
            description("synopsis\n A paragraph.\n .\n .\n Another.\n"),
            description("synopsis\n A paragraph.\n .\n"),
        ];
        BinaryPackageTester::new(ExtendedDescriptionContainsEmptyParagraph, pass, fail)
            .test_and_snapshot();
    }
}
