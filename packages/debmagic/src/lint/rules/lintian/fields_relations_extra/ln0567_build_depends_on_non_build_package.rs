use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::{SOURCE_DEPENDENCY_FIELDS, alternatives_of};

pub struct BuildDependsOnNonBuildPackage;

declare_rule! {
    /// A build dependency names a package that cannot be installed for a build.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch`
    /// against lintian's `fields/no-build-depends` list (interactive or
    /// unusable packages). Extra is the field and the alternative as written.
    BuildDependsOnNonBuildPackage,
    code = "LN0567",
    tag = "build-depends-on-non-build-package",
    default_selected = true,
    experimental = false,
}

static PACKAGES: LazyLock<Vec<String>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/no-build-depends"))
        .map(str::to_string)
        .collect()
});

impl SourceRule for BuildDependsOnNonBuildPackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages: Vec<String> = ctx
            .debian_control()
            .map(|control| {
                alternatives_of(control.source(), SOURCE_DEPENDENCY_FIELDS)
                    .into_iter()
                    .filter(|(_, alternative)| PACKAGES.iter().any(|p| p == alternative.package()))
                    .map(|(field, alternative)| format!("{field}: {}", alternative.text()))
                    .collect()
            })
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
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
            control(&[("Build-Depends", "debhelper, java-propose-classpath-doc")]),
            control(&[("Build-Conflicts", "java-propose-classpath")]),
        ];
        let fail = vec![
            control(&[("Build-Depends", "debhelper, java-propose-classpath")]),
            control(&[
                ("Build-Depends-Indep", "foo | java-propose-classpath (>= 1)"),
                ("Build-Depends-Arch", "java-propose-classpath:any"),
            ]),
        ];
        SourceTreeTester::new(BuildDependsOnNonBuildPackage, pass, fail).test_and_snapshot();
    }
}
