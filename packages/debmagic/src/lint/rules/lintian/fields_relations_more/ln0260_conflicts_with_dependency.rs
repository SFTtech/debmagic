use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::BINARY_DEP_FIELDS;
use super::overlap::{comma_separated, dependency_implies};

pub struct ConflictsWithDependency;

declare_rule! {
    /// `Conflicts` or `Breaks` names a package a dependency already requires.
    ///
    /// For each item of `Conflicts` and `Breaks`, in that order, every
    /// `Depends`, `Pre-Depends`, `Recommends`, and `Suggests` field is asked
    /// whether it implies that item. Implication here is the same package
    /// name, with architecture restrictions and `:arch` qualifiers at least as
    /// general on the dependency. An unversioned conflict is implied whatever
    /// version the dependency names. A versioned conflict is not reported:
    /// ordering two versions is not available. An alternative list implies a
    /// single package only when every alternative does. Extra is the
    /// dependency field and the conflict item as written. Applies to debs and
    /// udebs.
    ConflictsWithDependency,
    code = "LN0260",
    tag = "conflicts-with-dependency",
    default_selected = true,
    experimental = false,
}

const CONFLICT_FIELDS: &[&str] = &["Conflicts", "Breaks"];

impl BinaryPackageRule for ConflictsWithDependency {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = messages(ctx.installation_control().fields());
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn messages(fields: &ControlParagraph) -> Vec<String> {
    let mut hits = Vec::new();
    for conflict_field in CONFLICT_FIELDS {
        let Some(value) = fields.unfolded_value(conflict_field) else {
            continue;
        };
        let items = comma_separated(&value);
        for field in BINARY_DEP_FIELDS {
            let Some(relation) = fields.relation(field) else {
                continue;
            };
            for item in &items {
                if dependency_implies(&relation, item) {
                    hits.push(format!("{field} {item}"));
                }
            }
        }
    }
    hits
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
            fields(&[("Depends", "foo"), ("Breaks", "bar")]),
            // A version on the conflict is not ordered against the dependency.
            fields(&[("Depends", "libpng3"), ("Breaks", "libpng3 (<< 1.0)")]),
            fields(&[("Depends", "foo (>= 2)"), ("Conflicts", "foo (>= 1)")]),
            // An alternative does not require every name in it.
            fields(&[("Depends", "bar | foo"), ("Conflicts", "foo")]),
            // A restricted dependency does not imply an unrestricted conflict.
            fields(&[("Depends", "foo [amd64]"), ("Conflicts", "foo")]),
            fields(&[
                ("Depends", "foo (= 3) [!amd64 !i386]"),
                ("Conflicts", "foo [amd64 i386]"),
            ]),
            fields(&[("Depends", "foo:any"), ("Conflicts", "foo")]),
            fields(&[("Depends", "foo <nocheck>"), ("Conflicts", "foo")]),
            fields(&[("Conflicts", "foo")]),
        ];
        let fail = vec![
            fields(&[
                (
                    "Depends",
                    "relations (<< 3), foo (>> 2.0), foo (<< 2.2), libpng2, libpng3",
                ),
                ("Conflicts", "foobar (<< 5+5), foo, relations"),
                ("Breaks", "libpng3 (<< 1.0), libpng2"),
            ]),
            fields(&[("Depends", "conflict-dep"), ("Conflicts", "conflict-dep")]),
            fields(&[("Depends", "foo"), ("Conflicts", "foo | bar")]),
            fields(&[("Depends", "bar"), ("Breaks", "bar [alpha test]")]),
            fields(&[
                ("Pre-Depends", "foo"),
                ("Recommends", "foo"),
                ("Conflicts", "foo"),
            ]),
            fields(&[("Depends", "foo"), ("Conflicts", "foo")]).udeb(),
        ];
        BinaryPackageTester::new(ConflictsWithDependency, pass, fail).test_and_snapshot();
    }
}
