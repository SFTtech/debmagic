use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct IllegalMultiArchValue;

declare_rule! {
    /// An `Architecture: all` package declares `Multi-Arch: same`.
    ///
    /// Compares the unfolded `Architecture` and `Multi-Arch` values of the
    /// binary package's control file, which must both be declared. Extra is
    /// the architecture then the Multi-Arch value, separated by a space.
    /// Udebs are not skipped, as in lintian.
    IllegalMultiArchValue,
    code = "LN1055",
    tag = "illegal-multi-arch-value",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for IllegalMultiArchValue {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        if !fields.declares("Multi-Arch") {
            return;
        }
        let Some(multi) = fields.unfolded_value("Multi-Arch") else {
            return;
        };
        let Some(architecture) = fields.unfolded_value("Architecture") else {
            return;
        };
        if architecture == "all" && multi == "same" {
            ctx.diagnostic(Diagnostic::error(format!("{architecture} {multi}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(architecture: &str, multi: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
            .package("foo")
            .field("Architecture", architecture)
            .field("Multi-Arch", multi)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()).package("foo"),
            package("all", "foreign"),
            package("all", "allowed"),
            package("all", "no"),
            package("any", "same"),
            package("amd64", "same"),
            package("any", "same").udeb(),
        ];
        let fail = vec![package("all", "same"), package("all", "same").udeb()];
        BinaryPackageTester::new(IllegalMultiArchValue, pass, fail).test_and_snapshot();
    }
}
