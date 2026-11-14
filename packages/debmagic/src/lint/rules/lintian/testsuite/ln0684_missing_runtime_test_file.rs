use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::sections::{control_location, resolve, tests_control_sections};

pub struct MissingRuntimeTestFile;

declare_rule! {
    /// A test named in `Tests` does not exist in the source tree.
    ///
    /// Names are the unique entries of `Tests`, prefixed with `Tests-Directory`
    /// (default `debian/tests`; `.` adds no prefix). A path is missing when it
    /// does not resolve, following symbolic links inside the tree. Extra is the
    /// path. The Location is the `Tests` line. Applies to the Source tree and
    /// Source package.
    MissingRuntimeTestFile,
    code = "LN0684",
    tag = "missing-runtime-test-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MissingRuntimeTestFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let mut hits = Vec::new();
        for section in &sections {
            let line = section.line("Tests");
            for path in section.test_file_paths() {
                if resolve(files, &path).is_none() {
                    hits.push((path, line));
                }
            }
        }
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn(path).with_location(control_location(line)));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(control: &str, extra: Vec<(&'static str, TestFile)>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![("debian/tests/control", TestFile::new(control))];
        files.extend(extra);
        files
    }

    fn script() -> TestFile {
        TestFile::new("#!/bin/sh\n")
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no tests"))],
            tree("Test-Command: true\n", vec![]),
            tree(
                "Tests: one two\n",
                vec![
                    ("debian/tests/one", script()),
                    ("debian/tests/two", script()),
                ],
            ),
            tree(
                "Tests: alias\n",
                vec![
                    ("debian/tests/real", script()),
                    ("debian/tests/alias", TestFile::symlink("real")),
                ],
            ),
            tree(
                "Tests: check\nTests-Directory: .\n",
                vec![("check", script())],
            ),
            tree(
                "Tests: one\nTests-Directory: sub\n",
                vec![("sub/one", script())],
            ),
        ];
        let fail = vec![
            tree("Tests: gone\n", vec![]),
            tree(
                "Tests: one one two\n\nTests: three\n",
                vec![("debian/tests/one", script())],
            ),
            tree(
                "Tests: broken\n",
                vec![("debian/tests/broken", TestFile::symlink("nonexistent"))],
            ),
            tree("Tests: check\nTests-Directory: .\n", vec![]),
        ];
        SourceTreeTester::new(MissingRuntimeTestFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "Tests: one\n",
            vec![("debian/tests/one", script())],
        ))];
        let fail = vec![SourcePackageCase::from(tree("Tests: one\n", vec![]))];
        SourcePackageTester::new(MissingRuntimeTestFile, pass, fail).test_and_snapshot();
    }
}
