use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::BINARY_RELATION_FIELDS;

pub struct BadRelation;

declare_rule! {
    /// A relation in a binary relationship field does not parse as Policy 7.1 allows.
    ///
    /// Each alternative of `Depends`, `Pre-Depends`, `Recommends`, `Suggests`,
    /// `Conflicts`, `Provides`, `Enhances`, `Replaces`, and `Breaks` that keeps
    /// text after its package name, `:arch`, version, architecture restriction,
    /// and build profiles are read. Extra is the field and the alternative as
    /// written. Applies to debs and udebs.
    BadRelation,
    code = "LN0934",
    tag = "bad-relation",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BadRelation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages: Vec<String> = {
            let fields = ctx.installation_control().fields();
            BINARY_RELATION_FIELDS
                .iter()
                .filter_map(|field| Some((*field, fields.relation(field)?)))
                .flat_map(|(field, relation)| {
                    relation
                        .alternatives()
                        .filter(|alternative| !alternative.rest().is_empty())
                        .map(|alternative| format!("{field}: {}", alternative.text()))
                        .collect::<Vec<_>>()
                })
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
            fields(&[("Depends", "libc6 (>= 2.1), foo:any [amd64] <!nocheck>")]),
            fields(&[("Recommends", "a | b"), ("Provides", "foo (= 1.0)")]),
            // Build-Depends-style fields are not binary relations.
            fields(&[("Build-Depends", "foo (>< 1)")]),
            fields(&[("Replaces", "bar"), ("Breaks", "baz (<< 1)")]),
        ];
        let fail = vec![
            fields(&[("Depends", "another-package (>< 1.0)")]),
            fields(&[("Depends", "a b, c"), ("Conflicts", "d junk")]),
            fields(&[("Suggests", "e (1.0)")]),
        ];
        BinaryPackageTester::new(BadRelation, pass, fail).test_and_snapshot();
    }
}
