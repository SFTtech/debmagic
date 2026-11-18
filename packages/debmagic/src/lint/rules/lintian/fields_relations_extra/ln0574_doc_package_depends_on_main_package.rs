use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::alternatives_of;

pub struct DocPackageDependsOnMainPackage;

declare_rule! {
    /// A `<name>-doc` package depends on `<name>`.
    ///
    /// `Pre-Depends` and `Depends`, any alternative whose package name plus
    /// `-doc` is this package's name. Extra is the field. Applies to debs and
    /// udebs.
    DocPackageDependsOnMainPackage,
    code = "LN0574",
    tag = "doc-package-depends-on-main-package",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocPackageDependsOnMainPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let messages: Vec<String> = alternatives_of(
            ctx.installation_control().fields(),
            &["Depends", "Pre-Depends"],
        )
        .into_iter()
        .filter(|(_, alternative)| format!("{}-doc", alternative.package()) == package)
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
            binary(&[]).package("foo-doc"),
            binary(&[("Depends", "foo")]).package("foo"),
            binary(&[("Recommends", "foo")]).package("foo-doc"),
            binary(&[("Depends", "foo-common")]).package("foo-doc"),
            binary(&[("Depends", "foo-doc")]).package("foo-doc"),
        ];
        let fail = vec![
            binary(&[("Depends", "foo")]).package("foo-doc"),
            binary(&[("Pre-Depends", "bar | foo (>= 1)")]).package("foo-doc"),
            binary(&[("Depends", "foo:any")]).package("foo-doc").udeb(),
        ];
        BinaryPackageTester::new(DocPackageDependsOnMainPackage, pass, fail).test_and_snapshot();
    }
}
