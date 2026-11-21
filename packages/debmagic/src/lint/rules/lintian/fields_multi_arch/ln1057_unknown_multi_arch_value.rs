use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct UnknownMultiArchValue;

declare_rule! {
    /// A binary package declares a `Multi-Arch` value that Policy does not know.
    ///
    /// Known values are `no`, `foreign`, `allowed` and `same`, compared after
    /// unfolding. Extra is the package name then the unknown value, separated
    /// by a space. Packages without a `Multi-Arch` field are skipped. Udebs
    /// are not skipped, as in lintian.
    UnknownMultiArchValue,
    code = "LN1057",
    tag = "unknown-multi-arch-value",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnknownMultiArchValue {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        if !fields.declares("Multi-Arch") {
            return;
        }
        let Some(multi) = fields.unfolded_value("Multi-Arch") else {
            return;
        };
        if matches!(multi.as_str(), "no" | "foreign" | "allowed" | "same") {
            return;
        }
        let name = ctx.package_name().to_string();
        ctx.diagnostic(Diagnostic::error(format!("{name} {multi}")));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(multi: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
            .package("foo")
            .field("Multi-Arch", multi)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()).package("foo"),
            package("no"),
            package("foreign"),
            package("allowed"),
            package("same"),
            package("foreign").udeb(),
        ];
        let fail = vec![package("cheese"), package("Same"), package("cheese").udeb()];
        BinaryPackageTester::new(UnknownMultiArchValue, pass, fail).test_and_snapshot();
    }
}
