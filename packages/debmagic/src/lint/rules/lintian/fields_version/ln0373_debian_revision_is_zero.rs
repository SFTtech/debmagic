use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct DebianRevisionIsZero;

declare_rule! {
    /// The Version ends in `-0`, a zero Debian revision.
    ///
    /// Read from Installation control on a Binary package. Only the literal
    /// suffix `-0` is tested, so `1.0-0.1` is fine. Extra is the Version.
    /// The Subject has no Location.
    DebianRevisionIsZero,
    code = "LN0373",
    tag = "debian-revision-is-zero",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebianRevisionIsZero {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(version) = ctx
            .installation_control()
            .fields()
            .unfolded_value("Version")
        else {
            return;
        };
        if version.ends_with("-0") {
            ctx.diagnostic(Diagnostic::error(version));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn version(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Version", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            version("1.0-1"),
            version("1.0"),
            version("1.0-0.1"),
            version("1.0-10"),
        ];
        let fail = vec![version("1.0-0"), version("123.4-0")];
        BinaryPackageTester::new(DebianRevisionIsZero, pass, fail).test_and_snapshot();
    }
}
