use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct NewEssentialPackage;

declare_rule! {
    /// `Essential` is exactly `yes` and the package is not one lintian already
    /// knows.
    ///
    /// Installation control. The known set is lintian's `fields/essential`
    /// list, vendored at compile time. `no`, `maybe`, and any other value are
    /// not this Tag. Source packages are not checked (`essential-in-source-package`
    /// is a different Tag). Extra is empty. Udebs are checked too.
    NewEssentialPackage,
    code = "LN0348",
    tag = "new-essential-package",
    default_selected = true,
    experimental = false,
}

static KNOWN_ESSENTIAL: LazyLock<Vec<String>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/essential"))
        .map(str::to_string)
        .collect()
});

impl BinaryPackageRule for NewEssentialPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let value = ctx
            .installation_control()
            .fields()
            .unfolded_value("Essential");
        if value.as_deref() == Some("yes")
            && !KNOWN_ESSENTIAL
                .iter()
                .any(|name| name == ctx.package_name())
        {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn essential(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Essential", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            essential("no"),
            essential("maybe"),
            essential("Yes"),
            essential("yes").package("bash"),
            essential("yes").package("dpkg"),
            essential("yes").package("util-linux"),
        ];
        let fail = vec![
            essential("yes"),
            essential("yes").package("fields-essential"),
            essential("yes").package("another-version"),
            essential("yes").package("bash-doc").udeb(),
        ];
        BinaryPackageTester::new(NewEssentialPackage, pass, fail).test_and_snapshot();
    }
}
