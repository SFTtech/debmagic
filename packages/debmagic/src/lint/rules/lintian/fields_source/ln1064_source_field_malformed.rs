use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct SourceFieldMalformed;

declare_rule! {
    /// The `Source` field of a binary package is not a valid source name.
    ///
    /// Read from the Installation control, and only when `Source` is declared.
    /// Valid is a source package name, `[a-z0-9][-+.a-z0-9]+` (lintian's
    /// `$PKGNAME_REGEX`), optionally followed by a version in parentheses
    /// (`(1.0)`, `(1:1.0-2)`). Extra is the unfolded `Source` value. Applies
    /// to binary and udeb packages. The Source tree and Source package are
    /// not checked by this Rule.
    SourceFieldMalformed,
    code = "LN1064",
    tag = "source-field-malformed",
    default_selected = true,
    experimental = false,
}

static SOURCE_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[a-z0-9][-+.a-z0-9]+\s*(?:\((?:\d+:)?[-.+:a-zA-Z0-9~]+?(?:-[.+a-zA-Z0-9~]+)?\))?\s*$",
    )
    .expect("valid regex")
});

impl BinaryPackageRule for SourceFieldMalformed {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(source) = ctx.installation_control().fields().unfolded_value("Source") else {
            return;
        };
        if !SOURCE_FIELD.is_match(&source) {
            ctx.diagnostic(Diagnostic::error(source));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn source(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Source", value)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            source("example"),
            source("lib-x+y.z9"),
            source("ab"),
            source("example (1.0)"),
            source("example (1:1.0-2)"),
            source("example(2.0~rc1-3)"),
            source("example").udeb(),
        ];
        let fail = vec![
            source("Malformed"),
            source("a"),
            source("-example"),
            source("example_x"),
            source("example (1.0"),
            source("ex ample"),
            source("Example").udeb(),
            source("example (1.0) extra"),
        ];
        BinaryPackageTester::new(SourceFieldMalformed, pass, fail).test_and_snapshot();
    }
}
