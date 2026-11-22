use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct ConflictsWithVersion;

declare_rule! {
    /// A `Conflicts` alternative carries a version clause, such as
    /// `foo (<< 1.0)`. Such a clause is normally `Breaks`.
    ///
    /// Reads `Conflicts` of the Installation control. Every alternative with a
    /// version relation is reported, with extra the alternative as written. A
    /// version-less alternative is not. Udebs are checked too.
    ConflictsWithVersion,
    code = "LN1366",
    tag = "conflicts-with-version",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for ConflictsWithVersion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(relation) = ctx.installation_control().fields().relation("Conflicts") else {
            return;
        };
        let versioned: Vec<String> = relation
            .alternatives()
            .filter(|alternative| alternative.version().is_some())
            .map(|alternative| alternative.text().to_string())
            .collect();
        for text in versioned {
            ctx.diagnostic(Diagnostic::warn(text).with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn conflicts(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Conflicts", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            conflicts("foo"),
            conflicts("foo, bar | baz"),
            // Versions in other fields are not this Tag.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Breaks", "foo (<< 1.0)"),
        ];
        let fail = vec![
            conflicts("foo (<< 1.0)"),
            conflicts("foo, bar (>= 2) | baz (= 3)"),
            conflicts("foo (<< 1.0)").udeb(),
        ];
        BinaryPackageTester::new(ConflictsWithVersion, pass, fail).test_and_snapshot();
    }
}
