use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct UnknownEssentialValue;

declare_rule! {
    /// The `Essential` field of the Binary package has a value other than
    /// `yes` or `no`. Extra and Location are empty.
    ///
    /// Reads Installation control. The value is unfolded before the test and
    /// the comparison is case-sensitive, so `Yes` is not valid. An empty value
    /// is not valid. Udebs are checked too. Source packages are not this Tag.
    UnknownEssentialValue,
    code = "LN1051",
    tag = "unknown-essential-value",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnknownEssentialValue {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        if !fields.declares("Essential") {
            return;
        }
        let value = fields.unfolded_value("Essential").unwrap_or_default();
        if value != "yes" && value != "no" {
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
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            essential("yes"),
            essential("no"),
            essential("yes").udeb(),
        ];
        let fail = vec![
            essential("maybe"),
            essential("Yes"),
            essential(""),
            essential("true"),
            essential("NO").package("bash-doc").udeb(),
        ];
        BinaryPackageTester::new(UnknownEssentialValue, pass, fail).test_and_snapshot();
    }
}
