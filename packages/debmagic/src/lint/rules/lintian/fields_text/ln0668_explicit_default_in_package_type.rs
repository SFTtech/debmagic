use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct ExplicitDefaultInPackageType;

declare_rule! {
    /// `Package-Type` is exactly `deb`, which is the default.
    ///
    /// Reads the Installation control. Any other value, including `udeb` and
    /// `DEB`, is not this Tag. Extra is empty. Udebs are checked too.
    ExplicitDefaultInPackageType,
    code = "LN0668",
    tag = "explicit-default-in-package-type",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ExplicitDefaultInPackageType {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.installation_control().fields().get("Package-Type") == Some("deb") {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package_type(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Package-Type", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            package_type("udeb"),
            package_type("DEB"),
            package_type(""),
        ];
        let fail = vec![package_type("deb"), package_type("deb").udeb()];
        BinaryPackageTester::new(ExplicitDefaultInPackageType, pass, fail).test_and_snapshot();
    }
}
