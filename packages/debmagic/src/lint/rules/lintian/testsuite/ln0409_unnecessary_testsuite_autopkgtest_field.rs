use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::control::tests_control_present;

pub struct UnnecessaryTestsuiteAutopkgtestField;

declare_rule! {
    /// `Testsuite` lists `autopkgtest` while `debian/tests/control` exists.
    ///
    /// The field is split on commas. `debian/tests` must be a directory and
    /// `debian/tests/control` must be present. Extra is empty. The Location is
    /// `debian/control`. Applies to the Source tree and Source package.
    UnnecessaryTestsuiteAutopkgtestField,
    code = "LN0409",
    tag = "unnecessary-testsuite-autopkgtest-field",
    default_selected = true,
    experimental = false,
}

static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("regex"));

/// Line of `name:` . The hint's only text is that pointer.
fn field_line(text: &str, name: &str) -> Option<u32> {
    let prefix = format!("{}:", name.to_ascii_lowercase());
    text.lines().enumerate().find_map(|(index, line)| {
        line.to_ascii_lowercase()
            .starts_with(&prefix)
            .then_some(u32::try_from(index + 1).unwrap_or(u32::MAX))
    })
}

impl SourceRule for UnnecessaryTestsuiteAutopkgtestField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let Some(testsuite) = control.source().unfolded_value("Testsuite") else {
            return;
        };
        if !COMMA.split(&testsuite).any(|name| name == "autopkgtest") {
            return;
        }
        let line = {
            let Some(files) = ctx.files() else {
                return;
            };
            if !tests_control_present(files) {
                return;
            }
            files
                .get("debian/control")
                .and_then(|entry| files.read(entry).ok())
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .as_deref()
                .and_then(|text| field_line(text, "Testsuite"))
        };
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
            path: PathBuf::from("debian/control"),
            line,
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(testsuite: Option<&str>) -> Vec<(String, TestFile)> {
        let field = testsuite
            .map(|value| format!("Testsuite: {value}\n"))
            .unwrap_or_default();
        vec![
            (
                "debian/control".to_string(),
                TestFile::new(format!(
                    "Source: example\nMaintainer: Example <ex@example.com>\n{field}\n\
                     Package: example\nArchitecture: all\nDescription: example\n extra\n"
                )),
            ),
            ("debian/tests".to_string(), TestFile::directory()),
            (
                "debian/tests/control".to_string(),
                TestFile::new("Tests: smoke\nDepends: @\n"),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README".to_string(), TestFile::new("no packaging"))],
            tree(None),
            tree(Some("autopkgtest-pkg-perl")),
            vec![(
                "debian/control".to_string(),
                TestFile::new(
                    "Source: example\nMaintainer: Example <ex@example.com>\n\
                     Testsuite: autopkgtest\n\nPackage: example\nArchitecture: all\n\
                     Description: example\n extra\n",
                ),
            )],
        ];
        let fail = vec![
            tree(Some("autopkgtest")),
            tree(Some("autopkgtest, autopkgtest-pkg-perl")),
        ];
        SourceTreeTester::new(UnnecessaryTestsuiteAutopkgtestField, pass, fail).test_and_snapshot();
    }
}
