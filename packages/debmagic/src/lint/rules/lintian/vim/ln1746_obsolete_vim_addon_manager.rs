use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct ObsoleteVimAddonManager;

declare_rule! {
    /// A binary package's strong dependencies require `vim-addon-manager`.
    ///
    /// `vim-addon-manager` is not needed with debhelper: `dh_vim-addon` installs
    /// vim files where `:packadd` finds them (vim 8 and later). The strong
    /// relations are `Pre-Depends` and `Depends`, and the Rule asks whether
    /// they satisfy `vim-addon-manager`, so a package that only suggests it or
    /// that is satisfied by a virtual package does not count the same way.
    /// Reported as an info hint with no Extra and no Location. Binary packages
    /// only; udebs are checked too.
    ObsoleteVimAddonManager,
    code = "LN1746",
    tag = "obsolete-vim-addon-manager",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for ObsoleteVimAddonManager {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let strong = ctx
            .installation_control()
            .fields()
            .relations_and(&["Pre-Depends", "Depends"]);
        if strong.satisfies("vim-addon-manager") {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester};

    use super::*;

    fn depends(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, crate::lint::tester::TestFile)>::new())
            .field("Depends", value)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, crate::lint::tester::TestFile)>::new()),
            depends("vim"),
            depends("vim-addon-manager-extra"),
            // A suggested package is not a strong dependency.
            BinaryPackageCase::from(Vec::<(&str, crate::lint::tester::TestFile)>::new())
                .field("Suggests", "vim-addon-manager"),
        ];
        let fail = vec![
            depends("vim-addon-manager"),
            depends("vim, vim-addon-manager (>= 0.5)"),
            BinaryPackageCase::from(Vec::<(&str, crate::lint::tester::TestFile)>::new())
                .field("Pre-Depends", "vim-addon-manager"),
            depends("vim-addon-manager").udeb(),
        ];
        BinaryPackageTester::new(ObsoleteVimAddonManager, pass, fail).test_and_snapshot();
    }
}
