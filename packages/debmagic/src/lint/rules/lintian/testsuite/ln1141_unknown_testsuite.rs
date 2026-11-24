use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::sections::field_line;

pub struct UnknownTestsuite;

declare_rule! {
    /// `Testsuite` in `debian/control` names a test suite lintian does not know.
    ///
    /// The field is split on commas. Each distinct name missing from lintian's
    /// `testsuite/known-testsuites` (vendored) is one finding, in field order.
    /// Extra is the name. The Location is the `Testsuite` line of
    /// `debian/control`. An absent or empty field has no names. Applies to the
    /// Source tree and Source package.
    UnknownTestsuite,
    code = "LN1141",
    tag = "unknown-testsuite",
    default_selected = true,
    experimental = false,
}

static KNOWN: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/testsuite/known-testsuites")).collect()
});

static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("valid regex"));

impl SourceRule for UnknownTestsuite {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(testsuite) = ctx
            .debian_control()
            .and_then(|control| control.source().unfolded_value("Testsuite"))
        else {
            return;
        };
        // Perl's split drops trailing empty fields, and an empty value has none.
        let mut names: Vec<&str> = COMMA.split(&testsuite).collect();
        while names.last() == Some(&"") {
            names.pop();
        }
        let mut unknown: Vec<String> = Vec::new();
        for name in names {
            if !KNOWN.contains(&name) && !unknown.iter().any(|seen| seen == name) {
                unknown.push(name.to_string());
            }
        }
        if unknown.is_empty() {
            return;
        }
        let line = ctx
            .files()
            .and_then(|files| files.get("debian/control").map(|entry| (files, entry)))
            .and_then(|(files, entry)| files.read(entry).ok())
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .and_then(|text| field_line(&text, "Testsuite"));
        for name in unknown {
            ctx.diagnostic(Diagnostic::warn(name).with_location(Location {
                path: PathBuf::from("debian/control"),
                line,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(testsuite: Option<&str>) -> Vec<(String, TestFile)> {
        let field = testsuite
            .map(|value| format!("Testsuite: {value}\n"))
            .unwrap_or_default();
        vec![(
            "debian/control".to_string(),
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\n{field}\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(None),
            tree(Some("")),
            tree(Some("autopkgtest")),
            tree(Some("autopkgtest, autopkgtest-pkg-perl")),
            vec![("README".to_string(), TestFile::new("no packaging"))],
        ];
        let fail = vec![
            tree(Some("golf")),
            tree(Some("autopkgtest, golf")),
            tree(Some("golf, golf, hotel")),
        ];
        SourceTreeTester::new(UnknownTestsuite, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(Some("autopkgtest")))];
        let fail = vec![SourcePackageCase::from(tree(Some("golf")))];
        SourcePackageTester::new(UnknownTestsuite, pass, fail).test_and_snapshot();
    }
}
