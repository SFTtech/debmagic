use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct BadPackageName;

declare_rule! {
    /// The binary package name is not a valid Policy 5.6.7 package name.
    ///
    /// Installation control `Package`, unfolded. Valid is at least two
    /// characters, the first alphanumeric, the rest alphanumerics, `+`, `-`,
    /// or `.`; matching is case-insensitive (lintian's `$PKGNAME_REGEX` with
    /// `/i`). Uppercase names are valid here and are reported by
    /// `package-not-lowercase` (LN1060) instead. Covers deb, udeb, and ddeb.
    /// Extra is empty.
    BadPackageName,
    code = "LN1059",
    tag = "bad-package-name",
    default_selected = true,
    experimental = false,
}

/// lintian's `$PKGNAME_REGEX`, anchored, with `/i`.
static PKGNAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^[a-z0-9][-+.a-z0-9]+$").expect("valid regex"));

impl BinaryPackageRule for BadPackageName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(name) = ctx
            .installation_control()
            .fields()
            .unfolded_value("Package")
        else {
            return;
        };
        if !PKGNAME.is_match(&name) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).package(name)
    }

    #[test]
    fn test() {
        let pass = vec![
            package("libfoo1"),
            package("g++"),
            package("foo.bar-baz+1"),
            package("a1"),
            // Uppercase is package-not-lowercase's Tag, not this one.
            package("Foo"),
            package("libfoo-docs").udeb(),
        ];
        let fail = vec![
            package("a"),
            package("-foo"),
            package(".foo"),
            package("+foo"),
            package("foo_bar"),
            package("föo"),
            package("").udeb(),
            package("x_").udeb(),
        ];
        BinaryPackageTester::new(BadPackageName, pass, fail).test_and_snapshot();
    }
}
