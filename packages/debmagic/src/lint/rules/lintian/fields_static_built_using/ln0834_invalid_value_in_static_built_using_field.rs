use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::super::fields_built_using::first_invalid_clause;

pub struct InvalidValueInStaticBuiltUsingField;

declare_rule! {
    /// A `Static-Built-Using` clause is not `source (= version)`.
    ///
    /// Reads `Static-Built-Using` of the Installation control. Every clause
    /// must be a bare source name with an exact `=` version; an architecture,
    /// a profile, an OR clause, or any other operator is reported. Only the
    /// first such clause is reported; its extra is the clause as lintian
    /// prints it. Applies to debs and udebs.
    InvalidValueInStaticBuiltUsingField,
    code = "LN0834",
    tag = "invalid-value-in-static-built-using-field",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InvalidValueInStaticBuiltUsingField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let invalid = ctx
            .installation_control()
            .fields()
            .relation("Static-Built-Using")
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

    fn static_built_using(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Static-Built-Using", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            static_built_using(""),
            static_built_using("golang-github-mattn-go-xmpp (= 0.2.0-1)"),
            static_built_using("golang-example (= 1.0-1), golang-other (= 2:1.0~rc1+git-3)"),
            static_built_using("golang-example(=1.0)"),
            static_built_using("golang-example (= 1.0-1)").udeb(),
            // Built-Using is another field.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Built-Using", "foo (>= 1.0)"),
        ];
        let fail = vec![
            static_built_using("foo (>= 1.0)"),
            static_built_using("foo"),
            static_built_using("golang-example (= 1.0-1), foo (<< 2)"),
            static_built_using("foo (= 1.0) | bar (= 2.0)"),
            static_built_using("foo:any (= 1.0)"),
            static_built_using("foo (= 1.0) [amd64]"),
            static_built_using("foo (= 1.0) junk"),
            static_built_using("Foo (= 1.0)"),
            static_built_using("foo (>= 1.0)").udeb(),
        ];
        BinaryPackageTester::new(InvalidValueInStaticBuiltUsingField, pass, fail)
            .test_and_snapshot();
    }
}
