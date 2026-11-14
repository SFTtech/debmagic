use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::super::fields_relations_more::BINARY_DEP_FIELDS;
use super::alternatives_of;

pub struct DependsOnSpecificJavaDocPackage;

declare_rule! {
    /// A package depends on `classpath-doc` or `openjdk-<N>-doc`.
    ///
    /// `Depends`, `Pre-Depends`, `Recommends`, and `Suggests`; a package named
    /// `default-jdk-doc` is skipped. The name matches `classpath-doc` or
    /// contains `openjdk-<N>-doc`. Extra is the field. Applies to debs and
    /// udebs.
    DependsOnSpecificJavaDocPackage,
    code = "LN0575",
    tag = "depends-on-specific-java-doc-package",
    default_selected = true,
    experimental = false,
}

static OPENJDK_DOC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"openjdk-\d+-doc").expect("valid regex"));

impl BinaryPackageRule for DependsOnSpecificJavaDocPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name() == "default-jdk-doc" {
            return;
        }
        let messages: Vec<String> =
            alternatives_of(ctx.installation_control().fields(), BINARY_DEP_FIELDS)
                .into_iter()
                .filter(|(_, alternative)| {
                    alternative.package() == "classpath-doc"
                        || OPENJDK_DOC.is_match(alternative.package())
                })
                .map(|(field, _)| field)
                .collect();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::BinaryPackageTester;

    use super::super::testing::binary;
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            binary(&[]),
            binary(&[("Depends", "default-jdk-doc, openjdk-doc")]),
            binary(&[("Conflicts", "classpath-doc")]),
            binary(&[("Depends", "openjdk-17-doc")]).package("default-jdk-doc"),
        ];
        let fail = vec![
            binary(&[("Recommends", "openjdk-6-doc | default-jdk-doc")]),
            binary(&[
                ("Depends", "classpath-doc (>= 1)"),
                ("Suggests", "openjdk-17-doc"),
            ]),
            binary(&[("Pre-Depends", "foo-openjdk-8-doc-x")]).udeb(),
        ];
        BinaryPackageTester::new(DependsOnSpecificJavaDocPackage, pass, fail).test_and_snapshot();
    }
}
