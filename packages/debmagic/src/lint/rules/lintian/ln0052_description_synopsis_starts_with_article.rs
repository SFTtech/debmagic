use std::sync::LazyLock;

use regex::Regex;

static ARTICLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:an?|the)\s").expect("valid regex"));

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionSynopsisStartsWithArticle;

declare_rule! {
    /// The synopsis starts with an article.
    ///
    /// Case-insensitive `a`, `an` or `the` followed by whitespace at the
    /// start of the synopsis. Not checked when the synopsis is blank. Extra
    /// and Location are empty. Udebs are checked too.
    DescriptionSynopsisStartsWithArticle,
    code = "LN0052",
    tag = "description-synopsis-starts-with-article",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DescriptionSynopsisStartsWithArticle {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let synopsis = &description.synopsis;
        if synopsis.trim().is_empty() {
            return;
        }
        if ARTICLE.is_match(synopsis) {
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
            description("tool for examples"),
            description("another tool"),
            description("the"),
            description("theory of things"),
        ];
        let fail = vec![
            description("a tool"),
            description("An example tool"),
            description("THE tool"),
            description("the\ttool"),
        ];
        BinaryPackageTester::new(DescriptionSynopsisStartsWithArticle, pass, fail)
            .test_and_snapshot();
    }
}
