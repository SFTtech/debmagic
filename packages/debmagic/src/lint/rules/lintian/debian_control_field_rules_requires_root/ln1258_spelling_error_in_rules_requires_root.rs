use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct SpellingErrorInRulesRequiresRoot;

declare_rule! {
    /// The source paragraph of `debian/control` misspells `Rules-Requires-Root`.
    ///
    /// A field name in the source paragraph that matches `Rules?-Requires?-Roots?`
    /// case-insensitively, other than the exact spelling `Rules-Requires-Root`,
    /// is reported. The extra is the field name as written. The Location is
    /// `debian/control` with no line. Installable paragraphs are not checked.
    /// Applies to the Source tree and Source package.
    SpellingErrorInRulesRequiresRoot,
    code = "LN1258",
    tag = "spelling-error-in-rules-requires-root",
    default_selected = true,
    experimental = false,
}

static MISSPELLED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^Rules?-Requires?-Roots?$").expect("valid regex"));

impl SourceRule for SpellingErrorInRulesRequiresRoot {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let misspelled: Vec<String> = control
            .source()
            .names()
            .filter(|name| *name != "Rules-Requires-Root" && MISSPELLED.is_match(name))
            .map(str::to_string)
            .collect();
        for name in misspelled {
            ctx.diagnostic(Diagnostic::warn(name).with_location(Location {
                path: PathBuf::from("debian/control"),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(source_fields: &str) -> String {
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
{source_fields}\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n"
        )
    }

    fn package(source_fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(control(source_fields)))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no control"))],
            package(""),
            package("Rules-Requires-Root: no\n"),
            package("Rules-Requires-Root: binary-targets\n"),
            // Other field names are not this Tag.
            package("Rules-Requires: root\n"),
            // Only the source paragraph is checked.
            vec![(
                "debian/control",
                TestFile::new(format!(
                    "{}\nPackage: example-doc\nArchitecture: all\nRules-Require-Root: no\nDescription: x\n y\n",
                    control("Rules-Requires-Root: no\n")
                )),
            )],
        ];
        let fail = vec![
            package("Rules-Require-Root: no\n"),
            package("Rules-Requires-Root: no\nRules-Require-Roots: no\n"),
            package("rules-requires-root: no\n"),
        ];
        SourceTreeTester::new(SpellingErrorInRulesRequiresRoot, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            "Rules-Requires-Root: no\n",
        ))];
        let fail = vec![SourcePackageCase::from(package("Rules-Require-Root: no\n"))];
        SourcePackageTester::new(SpellingErrorInRulesRequiresRoot, pass, fail).test_and_snapshot();
    }
}
