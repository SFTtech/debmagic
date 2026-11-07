use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct NeedlessSuggestRecommendLibservletJava;

declare_rule! {
    /// `Recommends` or `Suggests` names a `libservlet<version>-java` package.
    ///
    /// Installation control. Every alternative is checked. Extra is the
    /// package name. Udebs are checked too.
    NeedlessSuggestRecommendLibservletJava,
    code = "LN0445",
    tag = "needless-suggest-recommend-libservlet-java",
    default_selected = true,
    experimental = false,
}

static SERVLET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"libservlet[\d.]+-java").expect("valid regex"));

const FIELDS: &[&str] = &["Recommends", "Suggests"];

impl BinaryPackageRule for NeedlessSuggestRecommendLibservletJava {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        let mut names = Vec::new();
        for field in FIELDS {
            let Some(relation) = fields.relation(field) else {
                continue;
            };
            for dependency in relation.dependencies() {
                for alternative in dependency {
                    if SERVLET.is_match(alternative.package()) {
                        names.push(alternative.package().to_string());
                    }
                }
            }
        }
        for name in names {
            ctx.diagnostic(Diagnostic::warn(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn relation(recommends: Option<&str>, suggests: Option<&str>) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(Vec::<(&str, TestFile)>::new());
        if let Some(value) = recommends {
            case = case.field("Recommends", value);
        }
        if let Some(value) = suggests {
            case = case.field("Suggests", value);
        }
        case
    }

    #[test]
    fn test() {
        let pass = vec![
            relation(None, None),
            relation(Some("default-jre"), Some("libfoo-java")),
            relation(None, Some("libservlet-java")),
        ];
        let fail = vec![
            relation(Some("libservlet3.0-java"), Some("libservlet2.5-java")),
            relation(Some("libservlet3.1-java | libother"), None),
            relation(None, Some("libservlet2.5-java")).udeb(),
        ];
        BinaryPackageTester::new(NeedlessSuggestRecommendLibservletJava, pass, fail)
            .test_and_snapshot();
    }
}
