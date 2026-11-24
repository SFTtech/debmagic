use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::sections::{control_location, tests_control_sections};

pub struct TestsuiteRestrictionsHasDeprecatedSkipNotInstallable;

declare_rule! {
    /// A paragraph of `debian/tests/control` uses the deprecated
    /// `skip-not-installable` restriction.
    ///
    /// The word is matched in the unfolded `Restrictions` of any paragraph. One
    /// finding for the file, however many paragraphs use it. Extra is empty, so
    /// the Location is `debian/tests/control` without a line. Applies to the
    /// Source tree and Source package.
    TestsuiteRestrictionsHasDeprecatedSkipNotInstallable,
    code = "LN1140",
    tag = "testsuite-restrictions-has-deprecated-skip-not-installable",
    default_selected = true,
    experimental = false,
}

static SKIP_NOT_INSTALLABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bskip-not-installable\b").expect("valid regex"));

impl SourceRule for TestsuiteRestrictionsHasDeprecatedSkipNotInstallable {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let used = sections
            .iter()
            .any(|section| SKIP_NOT_INSTALLABLE.is_match(&section.unfolded_value("Restrictions")));
        if used {
            ctx.diagnostic(Diagnostic::warn("").with_location(control_location(None)));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/tests/control", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("Tests: smoke\nRestrictions: needs-root\n"),
            tree("Tests: smoke\nRestrictions: skip-not-installablex\n"),
            tree("Tests: smoke\nRestrictions: superficial\n"),
            vec![("README", TestFile::new("no tests"))],
        ];
        let fail = vec![
            tree("Tests: smoke\nRestrictions: skip-not-installable\n"),
            tree(
                "Tests: a\nRestrictions: needs-root\n\nTests: b\nRestrictions: needs-root,\n skip-not-installable\n",
            ),
            tree(
                "Tests: a\nRestrictions: skip-not-installable\n\nTests: b\nRestrictions: skip-not-installable\n",
            ),
        ];
        SourceTreeTester::new(
            TestsuiteRestrictionsHasDeprecatedSkipNotInstallable,
            pass,
            fail,
        )
        .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("Tests: smoke\n"))];
        let fail = vec![SourcePackageCase::from(tree(
            "Tests: smoke\nRestrictions: skip-not-installable\n",
        ))];
        SourcePackageTester::new(
            TestsuiteRestrictionsHasDeprecatedSkipNotInstallable,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
