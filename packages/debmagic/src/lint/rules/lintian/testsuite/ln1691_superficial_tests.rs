use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::sections::{control_location, tests_control_sections};

pub struct SuperficialTests;

declare_rule! {
    /// Every paragraph of `debian/tests/control` has a `superficial`
    /// restriction, and the Source package names no testsuite other than
    /// the unknown ones (no `Testsuite` field, or only unknown names). Reported
    /// at info severity with an empty extra and the `debian/tests/control`
    /// Location.
    ///
    /// The `Testsuite` names are split on commas as lintian does, dropping
    /// trailing empty names. An unknown name is one missing from
    /// `testsuite/known-testsuites`. A paragraph is superficial when its
    /// `Restrictions` contain the word `superficial`. A file with no paragraph
    /// is not reported.
    SuperficialTests,
    code = "LN1691",
    tag = "superficial-tests",
    default_selected = false,
    experimental = false,
}

static KNOWN: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/testsuite/known-testsuites")).collect()
});
static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("valid regex"));
static SUPERFICIAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bsuperficial\b").expect("valid regex"));

impl SourceRule for SuperficialTests {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        // Perl's split drops trailing empty fields, and an empty value has none.
        let testsuite = ctx
            .debian_control()
            .and_then(|control| control.source().unfolded_value("Testsuite"))
            .unwrap_or_default();
        let mut split: Vec<&str> = COMMA.split(&testsuite).collect();
        while split.last() == Some(&"") {
            split.pop();
        }
        let names: Vec<String> = split.into_iter().map(str::to_string).collect();
        let unknown = names
            .iter()
            .filter(|name| !KNOWN.contains(&name.as_str()))
            .count();

        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        if sections.is_empty() {
            return;
        }
        let all_superficial = sections
            .iter()
            .all(|section| SUPERFICIAL.is_match(&section.unfolded_value("Restrictions")));
        if all_superficial && (names.is_empty() || names.len() == unknown) {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(control_location(None)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(testsuite: Option<&str>, tests: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n\
                 {}\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                testsuite
                    .map(|testsuite| format!("Testsuite: {testsuite}\n"))
                    .unwrap_or_default()
            )),
        )];
        if let Some(tests) = tests {
            files.push(("debian/tests/control", TestFile::new(tests)));
        }
        files
    }

    const SUPERFICIAL_TEST: &str = "Tests: smoke\nRestrictions: superficial\n";
    const THOROUGH_TEST: &str = "Tests: full\n";

    #[test]
    fn test() {
        let pass = vec![
            // No tests control, or no paragraph.
            tree(None, None),
            tree(None, Some("# only a comment\n")),
            // A thorough paragraph beside a superficial one.
            tree(
                None,
                Some(format!("{SUPERFICIAL_TEST}\n{THOROUGH_TEST}").as_str()),
            ),
            tree(None, Some(THOROUGH_TEST)),
            // A known Testsuite is not unknown, so the superficial tests are
            // left to the Testsuite rules.
            tree(Some("autopkgtest"), Some(SUPERFICIAL_TEST)),
        ];
        let fail = vec![
            tree(None, Some(SUPERFICIAL_TEST)),
            tree(Some("unknown-suite"), Some(SUPERFICIAL_TEST)),
            tree(
                None,
                Some(
                    format!(
                        "{SUPERFICIAL_TEST}\nTests: other\nRestrictions: superficial, needs-root\n"
                    )
                    .as_str(),
                ),
            ),
        ];
        SourceTreeTester::new(SuperficialTests, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(None, Some(THOROUGH_TEST)))];
        let fail = vec![SourcePackageCase::from(tree(None, Some(SUPERFICIAL_TEST)))];
        SourcePackageTester::new(SuperficialTests, pass, fail).test_and_snapshot();
    }
}
