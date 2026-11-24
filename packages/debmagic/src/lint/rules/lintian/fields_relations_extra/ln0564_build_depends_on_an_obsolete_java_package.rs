use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::fields_relations_more::SOURCE_RELATION_FIELDS;
use super::alternatives_of;

pub struct BuildDependsOnAnObsoleteJavaPackage;

declare_rule! {
    /// A source relation names the obsolete `java-compiler` package.
    ///
    /// Build-dependency and build-conflict fields of the source paragraph;
    /// any alternative whose package is `java-compiler` counts. Extra is the
    /// package name.
    BuildDependsOnAnObsoleteJavaPackage,
    code = "LN0564",
    tag = "build-depends-on-an-obsolete-java-package",
    default_selected = true,
    experimental = false,
}

impl SourceRule for BuildDependsOnAnObsoleteJavaPackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages: Vec<String> = ctx
            .debian_control()
            .map(|control| {
                alternatives_of(control.source(), SOURCE_RELATION_FIELDS)
                    .into_iter()
                    .filter(|(_, alternative)| alternative.package() == "java-compiler")
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
            control(&[("Build-Depends", "default-jdk, java-compiler-extra")]),
        ];
        let fail = vec![
            control(&[("Build-Depends", "java-compiler | default-jdk")]),
            control(&[
                ("Build-Depends-Indep", "java-compiler (>= 1.5)"),
                ("Build-Conflicts", "java-compiler:any"),
            ]),
        ];
        SourceTreeTester::new(BuildDependsOnAnObsoleteJavaPackage, pass, fail).test_and_snapshot();
    }
}
