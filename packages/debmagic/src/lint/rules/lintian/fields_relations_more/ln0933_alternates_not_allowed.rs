use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{BINARY_DEP_FIELDS, BINARY_RELATION_FIELDS};

pub struct AlternatesNotAllowed;

declare_rule! {
    /// A relationship field other than the dependency fields contains `|`.
    ///
    /// Checked on the unfolded value of `Conflicts`, `Provides`, `Enhances`,
    /// `Replaces`, and `Breaks`. `Depends`, `Pre-Depends`, `Recommends`, and
    /// `Suggests` may use alternatives and are skipped. Extra is the field name.
    /// Applies to debs and udebs.
    AlternatesNotAllowed,
    code = "LN0933",
    tag = "alternates-not-allowed",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for AlternatesNotAllowed {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages: Vec<String> = {
            let fields = ctx.installation_control().fields();
            BINARY_RELATION_FIELDS
                .iter()
                .copied()
                .filter(|field| !BINARY_DEP_FIELDS.contains(field))
                .filter(|field| {
                    fields
                        .unfolded_value(field)
                        .is_some_and(|value| value.contains('|'))
                })
                .map(str::to_string)
                .collect()
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn fields(items: &[(&str, &str)]) -> BinaryPackageCase {
        items.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            fields(&[]),
            fields(&[("Depends", "foo | bar")]),
            fields(&[("Pre-Depends", "foo | bar"), ("Recommends", "a | b")]),
            fields(&[("Suggests", "a | b"), ("Conflicts", "foo, bar")]),
            fields(&[("Provides", "foo"), ("Breaks", "bar (<< 1)")]),
        ];
        let fail = vec![
            fields(&[("Conflicts", "foo | bar")]),
            fields(&[("Provides", "foo | bar")]),
            fields(&[
                ("Enhances", "a|b"),
                ("Replaces", "c | d"),
                ("Breaks", "e | f"),
            ]),
        ];
        BinaryPackageTester::new(AlternatesNotAllowed, pass, fail).test_and_snapshot();
    }
}
