use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

use super::sections::{field_line, resolve};

pub struct MissingTestsControl;

declare_rule! {
    /// `Testsuite` lists `autopkgtest` while `debian/tests/control` is absent.
    ///
    /// The field is split on commas. `debian/tests/control` counts as present
    /// when it resolves, through symbolic links inside the tree, to an entry
    /// and `debian/tests` is a directory; as in lintian, a missing `control`
    /// file is the trigger even when the directory exists. Extra is empty. The
    /// Location is the `Testsuite` line of `debian/control`. Applies to the
    /// Source tree and Source package.
    MissingTestsControl,
    code = "LN0995",
    tag = "missing-tests-control",
    default_selected = true,
    experimental = false,
}

static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("regex"));

fn tests_control_present(files: &FileTree) -> bool {
    let directory =
        resolve(files, "debian/tests").is_some_and(|entry| entry.kind() == EntryKind::Directory);
    directory && resolve(files, "debian/tests/control").is_some()
}

impl SourceRule for MissingTestsControl {
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
            if tests_control_present(files) {
                return;
            }
            files
                .get("debian/control")
                .and_then(|entry| files.read(entry).ok())
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .as_deref()
                .and_then(|text| field_line(text, "Testsuite"))
        };
        ctx.diagnostic(Diagnostic::error("").with_location(Location {
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
        vec![(
            "debian/control".to_string(),
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\n{field}\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    fn with_tests(testsuite: Option<&str>, tests: TestFile) -> Vec<(String, TestFile)> {
        let mut files = tree(testsuite);
        files.push(("debian/tests".to_string(), TestFile::directory()));
        files.push(("debian/tests/control".to_string(), tests));
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README".to_string(), TestFile::new("no packaging"))],
            tree(None),
            tree(Some("autopkgtest-pkg-perl")),
            with_tests(Some("autopkgtest"), TestFile::new("Tests: smoke\n")),
            with_tests(
                Some("autopkgtest, autopkgtest-pkg-perl"),
                TestFile::new("Tests: smoke\n"),
            ),
            vec![
                (
                    "debian/control".to_string(),
                    TestFile::new("Source: example\nTestsuite: autopkgtest\n\nPackage: example\n"),
                ),
                (
                    "debian/tests".to_string(),
                    TestFile::symlink("../tests-real"),
                ),
                ("tests-real".to_string(), TestFile::directory()),
                (
                    "tests-real/control".to_string(),
                    TestFile::new("Tests: smoke\n"),
                ),
            ],
        ];
        let fail = vec![
            tree(Some("autopkgtest")),
            tree(Some("autopkgtest, autopkgtest-pkg-perl")),
            // The directory exists but holds only a script, as in the recipe.
            vec![
                (
                    "debian/control".to_string(),
                    TestFile::new("Source: example\nTestsuite: autopkgtest\n\nPackage: example\n"),
                ),
                ("debian/tests".to_string(), TestFile::directory()),
                (
                    "debian/tests/test".to_string(),
                    TestFile::new("#!/bin/sh\n"),
                ),
            ],
            vec![
                (
                    "debian/control".to_string(),
                    TestFile::new("Source: example\nTestsuite: autopkgtest\n\nPackage: example\n"),
                ),
                (
                    "debian/tests".to_string(),
                    TestFile::new("not a directory\n"),
                ),
            ],
        ];
        SourceTreeTester::new(MissingTestsControl, pass, fail).test_and_snapshot();
    }
}
