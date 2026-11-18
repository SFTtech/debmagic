use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct BugsFieldDoesNotReferToDebianInfrastructure;

declare_rule! {
    /// The `Bugs` field does not name a `.debian.org` host.
    ///
    /// Reads the unfolded `Bugs` of the Installation control. The value must
    /// contain the literal `.debian.org` (case-sensitive) somewhere, or
    /// reportbug cannot file bugs. A `-dbgsym` package is exempt. Extra is the
    /// unfolded value. Location is empty. Udebs are checked too.
    BugsFieldDoesNotReferToDebianInfrastructure,
    code = "LN0801",
    tag = "bugs-field-does-not-refer-to-debian-infrastructure",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BugsFieldDoesNotReferToDebianInfrastructure {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name().ends_with("-dbgsym") {
            return;
        }
        let Some(bugs) = ctx.installation_control().fields().unfolded_value("Bugs") else {
            return;
        };
        if !bugs.contains(".debian.org") {
            ctx.diagnostic(Diagnostic::warn(bugs));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn bugs(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Bugs", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            bugs("https://bugs.debian.org/"),
            bugs("https://bugs.debian.org/src/example"),
            bugs("debbugs://bugs.debian.org/"),
            bugs("https://upstream.org/").package("example-dbgsym"),
            bugs("https://upstream.org/")
                .udeb()
                .package("example-dbgsym"),
        ];
        let fail = vec![
            bugs("https://upstream.org/"),
            bugs("mailto:bugs@example.com"),
            bugs("https://debian-org.example.com/"),
            bugs("https://upstream.org/").udeb(),
        ];
        BinaryPackageTester::new(BugsFieldDoesNotReferToDebianInfrastructure, pass, fail)
            .test_and_snapshot();
    }
}
