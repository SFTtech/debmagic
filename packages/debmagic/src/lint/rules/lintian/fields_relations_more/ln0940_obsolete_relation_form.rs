use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::BINARY_RELATION_FIELDS;

pub struct ObsoleteRelationForm;

declare_rule! {
    /// A versioned relation uses the obsolete `<` or `>` operator.
    ///
    /// In Policy 7.1 `<` and `>` mean `<=` and `>=`, not the strict `<<` and
    /// `>>`. Checked on every alternative of the binary relationship fields
    /// `Depends` through `Breaks`. Extra is the field and the alternative as
    /// written. Applies to debs and udebs.
    ObsoleteRelationForm,
    code = "LN0940",
    tag = "obsolete-relation-form",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ObsoleteRelationForm {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages: Vec<String> = {
            let fields = ctx.installation_control().fields();
            BINARY_RELATION_FIELDS
                .iter()
                .filter_map(|field| Some((*field, fields.relation(field)?)))
                .flat_map(|(field, relation)| {
                    relation
                        .alternatives()
                        .filter(|alternative| {
                            alternative.version().is_some_and(|version| {
                                matches!(version.operator.as_str(), "<" | ">")
                            })
                        })
                        .map(|alternative| format!("{field}: {}", alternative.text()))
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
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
            fields(&[(
                "Depends",
                "libc6 (>= 2.1), foo (<< 2), foo (<= 2), foo (= 1)",
            )]),
            fields(&[
                ("Conflicts", "libsqlite3-0 (<< 3.6.12)"),
                ("Breaks", "x (>> 1)"),
            ]),
            fields(&[("Build-Depends", "foo (< 1)")]),
        ];
        let fail = vec![
            fields(&[("Conflicts", "libsqlite3-0 (< 3.6.12)")]),
            fields(&[("Depends", "a (> 1), b (<< 2)"), ("Replaces", "c (< 3)")]),
        ];
        BinaryPackageTester::new(ObsoleteRelationForm, pass, fail).test_and_snapshot();
    }
}
