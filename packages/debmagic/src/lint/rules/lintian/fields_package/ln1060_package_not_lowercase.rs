use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageNotLowercase;

declare_rule! {
    /// The binary package name contains an uppercase ASCII letter.
    ///
    /// Installation control `Package`, unfolded; any byte in `A` to `Z` counts
    /// (lintian's `/[A-Z]/`, case-sensitive). The name is not otherwise
    /// validated, so a name that is also invalid gets `bad-package-name`
    /// (LN1059) as well. Covers deb, udeb, and ddeb. Extra is empty.
    PackageNotLowercase,
    code = "LN1060",
    tag = "package-not-lowercase",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageNotLowercase {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(name) = ctx
            .installation_control()
            .fields()
            .unfolded_value("Package")
        else {
            return;
        };
        if name.bytes().any(|byte| byte.is_ascii_uppercase()) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).package(name)
    }

    #[test]
    fn test() {
        let pass = vec![
            package("libfoo1"),
            package("foo-docs"),
            package("g++"),
            package("libfoo").udeb(),
        ];
        let fail = vec![
            package("Foo"),
            package("foo_BAR"),
            package("libFoo1"),
            package("FOO").udeb(),
            package("x-Y").udeb(),
        ];
        BinaryPackageTester::new(PackageNotLowercase, pass, fail).test_and_snapshot();
    }
}
