use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct EmptyField;

declare_rule! {
    /// A field of the Binary package's control file has an empty value, or a
    /// value that is only whitespace. Reports each such field, in file order,
    /// with the field name as extra.
    ///
    /// Reads Installation control. The value is unfolded and trimmed before
    /// the test, as lintian's `value` is. A field declared with no value
    /// counts. Udebs are checked too. Location is empty: the Subject is the
    /// location.
    EmptyField,
    code = "LN1050",
    tag = "empty-field",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for EmptyField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        let empty: Vec<String> = fields
            .names()
            .filter(|name| fields.unfolded_value(name).unwrap_or_default().is_empty())
            .map(str::to_string)
            .collect();
        for name in empty {
            ctx.diagnostic(Diagnostic::warn(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn field(name: &str, value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field(name, value)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            field("Section", "misc"),
            field("Description", "example\n  extra"),
            field("Essential", "yes"),
        ];
        let fail = vec![
            field("Section", ""),
            field("Section", "   "),
            field("Section", "").udeb(),
            field("Homepage", " \n "),
        ];
        BinaryPackageTester::new(EmptyField, pass, fail).test_and_snapshot();
    }
}
