use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::super::fields_relations_more::{BINARY_RELATION_FIELDS, SOURCE_RELATION_FIELDS};
use super::alternatives_of;

pub struct BadVersionInRelation;

declare_rule! {
    /// A relation constrains a package to a version string that is not a Debian version.
    ///
    /// Every alternative with a `(op version)` constraint in the source
    /// relationship fields (build dependencies and conflicts) or the binary
    /// relationship fields (`Depends` through `Breaks`). The version is
    /// checked as dpkg's `version_check`: an optional numeric epoch, an
    /// upstream version starting with a digit, and an optional revision.
    /// Extra is the field and the alternative as written. Applies to debs,
    /// udebs, the Source tree, and the Source package.
    BadVersionInRelation,
    code = "LN0568",
    tag = "bad-version-in-relation",
    default_selected = true,
    experimental = false,
}

static VERSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:\d+:)?\d[A-Za-z0-9.+:~-]*?(?:-[A-Za-z0-9+.~]+)?$").expect("valid regex")
});

impl SourceRule for BadVersionInRelation {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx
            .debian_control()
            .map(|control| messages(control.source(), SOURCE_RELATION_FIELDS))
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

impl BinaryPackageRule for BadVersionInRelation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = messages(ctx.installation_control().fields(), BINARY_RELATION_FIELDS);
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn messages(fields: &ControlParagraph, names: &[&str]) -> Vec<String> {
    alternatives_of(fields, names)
        .into_iter()
        .filter(|(_, alternative)| {
            alternative
                .version()
                .is_some_and(|constraint| !VERSION.is_match(&constraint.version))
        })
        .map(|(field, alternative)| format!("{field}: {}", alternative.text()))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageTester, SourceTreeTester, TestFile};

    use super::super::testing::{binary, control};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[(
                "Build-Depends",
                "foo (>= 1:2.0~rc1-1+b1), bar (= 1.0), baz (<< 2:0), qux",
            )]),
        ];
        let fail = vec![
            control(&[("Build-Depends", "g++ (>= s4.0), ok (>= 1)")]),
            control(&[
                ("Build-Depends-Arch", "foo | bar (= 1_0)"),
                ("Build-Conflicts", "baz (<< 1/2)"),
            ]),
            control(&[("Build-Depends-Indep", "foo (>= :1)")]),
            control(&[("Build-Depends", "foo (>= ${source:Version})")]),
        ];
        SourceTreeTester::new(BadVersionInRelation, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let pass = vec![
            binary(&[]),
            binary(&[("Depends", "libc6 (>= 2.34), foo (= 1.0-1~bpo12+1)")]),
            binary(&[("Provides", "virtual (= 1.0)")]),
        ];
        let fail = vec![
            binary(&[("Depends", "foo (>= 1.0), bar (>= x1)")]),
            binary(&[
                ("Breaks", "old (<< 1.0_beta)"),
                ("Replaces", "old (<< a-1)"),
            ]),
            binary(&[("Suggests", "foo (= -1)")]).udeb(),
        ];
        BinaryPackageTester::new(BadVersionInRelation, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
