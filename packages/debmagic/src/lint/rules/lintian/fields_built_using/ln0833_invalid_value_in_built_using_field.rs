use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::first_invalid_clause;

pub struct InvalidValueInBuiltUsingField;

declare_rule! {
    /// A `Built-Using` clause is not `source (= version)`.
    ///
    /// Reads `Built-Using` of the Installation control. Every clause must be a
    /// bare source name with an exact `=` version; an architecture, a profile,
    /// an OR clause, or any other operator is reported. Only the first such
    /// clause is reported; its extra is the clause as lintian prints it.
    /// Applies to debs and udebs.
    InvalidValueInBuiltUsingField,
    code = "LN0833",
    tag = "invalid-value-in-built-using-field",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InvalidValueInBuiltUsingField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let invalid = ctx
            .installation_control()
            .fields()
            .relation("Built-Using")
            .as_ref()
            .and_then(first_invalid_clause);
        if let Some(clause) = invalid {
            ctx.diagnostic(Diagnostic::error(clause));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn built_using(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Built-Using", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            built_using(""),
            built_using("golang-example (= 1.0-1)"),
            built_using("golang-example (= 1.0-1), gcc-12 (= 12.2.0-14)"),
            built_using("golang-example (= 2:1.0~rc1+git-3)"),
            built_using("golang-example(=1.0)"),
            built_using("golang-example (=\n 1.0)"),
            built_using("golang-example (= 1.0-1)").udeb(),
            // Relation fields other than Built-Using are not read.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Depends", "foo (>= 1.0)"),
        ];
        let fail = vec![
            built_using("foo (>= 1.0)"),
            built_using("foo"),
            built_using("golang-example (= 1.0-1), foo (<< 2)"),
            built_using("foo (= 1.0) | bar (= 2.0)"),
            built_using("foo:any (= 1.0)"),
            built_using("foo (= 1.0) [amd64]"),
            built_using("foo (= 1.0) junk"),
            built_using("Foo (= 1.0)"),
            built_using("foo (>= 1.0)").udeb(),
        ];
        BinaryPackageTester::new(InvalidValueInBuiltUsingField, pass, fail).test_and_snapshot();
    }
}
