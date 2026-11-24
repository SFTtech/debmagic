use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::fields_relations_more::SOURCE_RELATION_FIELDS;
use super::alternatives_of;

pub struct BuildDependsOnSpecificJavaDocPackage;

declare_rule! {
    /// A source relation names `openjdk-<N>-doc` or `classpath-doc`.
    ///
    /// Build-dependency and build-conflict fields of the source paragraph.
    /// Extra is the package name.
    BuildDependsOnSpecificJavaDocPackage,
    code = "LN0565",
    tag = "build-depends-on-specific-java-doc-package",
    default_selected = true,
    experimental = false,
}

static OPENJDK_DOC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^openjdk-\d+-doc$").expect("valid regex"));

impl SourceRule for BuildDependsOnSpecificJavaDocPackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages: Vec<String> = ctx
            .debian_control()
            .map(|control| {
                alternatives_of(control.source(), SOURCE_RELATION_FIELDS)
                    .into_iter()
                    .filter(|(_, alternative)| {
                        OPENJDK_DOC.is_match(alternative.package())
                            || alternative.package() == "classpath-doc"
                    })
                    .map(|(_, alternative)| alternative.package().to_string())
                    .collect()
            })
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::super::testing::control;
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[(
                "Build-Depends",
                "default-jdk-doc, openjdk-doc, openjdk-17-jdk",
            )]),
        ];
        let fail = vec![
            control(&[("Build-Depends-Indep", "openjdk-6-doc | default-jdk-doc")]),
            control(&[
                ("Build-Depends", "classpath-doc (>= 1)"),
                ("Build-Conflicts", "openjdk-17-doc"),
            ]),
        ];
        SourceTreeTester::new(BuildDependsOnSpecificJavaDocPackage, pass, fail).test_and_snapshot();
    }
}
