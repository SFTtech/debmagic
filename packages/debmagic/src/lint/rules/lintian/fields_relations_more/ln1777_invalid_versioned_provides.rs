use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct InvalidVersionedProvides;

declare_rule! {
    /// A `Provides` alternative has a version constraint other than `=`.
    ///
    /// Every alternative of the binary `Provides` field that carries an
    /// `(op version)` whose operator is not `=`. The obsolete `<` and `>` count
    /// as invalid too. An alternative without a constraint, or with `=`, is
    /// fine. Extra is the alternative as written, without its separators and
    /// keeping any `[arch]` or `<profile>` restriction. Applies to debs, udebs,
    /// and ddebs.
    InvalidVersionedProvides,
    code = "LN1777",
    tag = "invalid-versioned-provides",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InvalidVersionedProvides {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = messages(ctx.installation_control().fields());
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn messages(fields: &ControlParagraph) -> Vec<String> {
    let Some(relation) = fields.relation("Provides") else {
        return Vec::new();
    };
    relation
        .alternatives()
        .filter(|alternative| {
            alternative
                .version()
                .is_some_and(|constraint| constraint.operator != "=")
        })
        .map(|alternative| alternative.text().to_string())
        .collect()
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
            fields(&[("Provides", "foo")]),
            fields(&[("Provides", "foo (= 1.0)")]),
            fields(&[("Provides", "foo (=1.0), bar, baz [i386]")]),
            // Not a constraint: the version has no operator.
            fields(&[("Provides", "foo (1.0)")]),
            // Only Provides is checked.
            fields(&[("Depends", "foo (>= 1.0)")]),
            fields(&[("Conflicts", "foo (<< 1.0)")]),
            fields(&[("Replaces", "foo (> 1.0)")]),
            fields(&[("Provides", "foo, , bar")]),
        ];
        let fail = vec![
            fields(&[("Provides", "foo (>= 1.0)")]),
            fields(&[("Provides", "foo (<< 1.0)")]),
            fields(&[("Provides", "foo (<= 1.0)")]),
            fields(&[("Provides", "foo (>> 1.0)")]),
            // The obsolete operators are invalid too.
            fields(&[("Provides", "foo (< 1.0)")]),
            fields(&[("Provides", "foo (> 1.0)")]),
            // Each bad alternative is reported; the good one is not.
            fields(&[("Provides", "good (= 1.0), bad (>= 2.0) | other")]),
            fields(&[("Provides", "arch-specific (>= 1.0) [i386]")]),
            fields(&[("Provides", "foo (>= 1.0)")]).udeb(),
        ];
        BinaryPackageTester::new(InvalidVersionedProvides, pass, fail).test_and_snapshot();
    }
}
