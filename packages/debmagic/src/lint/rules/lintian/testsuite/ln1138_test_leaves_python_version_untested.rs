use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

use super::python::{depends_on_python3_all, scan};
use super::sections::{resolve, tests_control_sections};

pub struct TestLeavesPythonVersionUntested;

declare_rule! {
    /// A test script leaves the Python versions untested: its paragraph of
    /// `debian/tests/control` depends on `python3-all` (or `-dev`, `-dbg`) but
    /// the script never asks `py3versions` for the supported versions with
    /// `-s` or `--supported`.
    ///
    /// For each paragraph, each `Tests` script (under `Tests-Directory`) that
    /// resolves to a regular file or hardlink inside the tree is checked. A
    /// missing or non-regular script is skipped. The Depends is expanded for
    /// `@builddeps@` and `@recommends@` before the check. Extra is empty, so the
    /// Location is the script path without a line. Applies to the Source tree
    /// and Source package.
    TestLeavesPythonVersionUntested,
    code = "LN1138",
    tag = "test-leaves-python-version-untested",
    default_selected = true,
    experimental = false,
}

impl SourceRule for TestLeavesPythonVersionUntested {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control().cloned() else {
            return;
        };
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let mut hits: Vec<PathBuf> = Vec::new();
        for section in &sections {
            if !depends_on_python3_all(section, &control) {
                continue;
            }
            let calls = scan(files, section).calls;
            for path in section.test_file_paths() {
                let Some(entry) = resolve(files, &path) else {
                    continue;
                };
                if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                    continue;
                }
                let queries_supported = calls
                    .iter()
                    .any(|call| call.location.path == entry.path() && call.asks_supported());
                if !queries_supported {
                    hits.push(entry.path().to_path_buf());
                }
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
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

    fn tree(depends: &str, script: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            (
                "debian/control",
                TestFile::new(
                    "Source: example\nMaintainer: Example <ex@example.com>\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
                ),
            ),
            (
                "debian/tests/control",
                TestFile::new(format!("Tests: some-python\nDepends: {depends}\n")),
            ),
            ("debian/tests/some-python", TestFile::new(script)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("python3-all", "py3versions -s\n"),
            tree("python3-all-dev, python3", "py3versions --supported\n"),
            tree("python3", "py3versions -r\n"),
            tree("@builddeps@", "echo nothing\n"),
            tree("python3-all", "echo nothing\n")
                .into_iter()
                .filter(|(path, _)| *path != "debian/tests/some-python")
                .collect(),
        ];
        let fail = vec![
            tree("python3-all", "py3versions -r\n"),
            tree("python3-all", "py3versions -i\n"),
            tree("python3-all:any", "py3versions -i\n"),
            tree("python3-all-dbg", "echo nothing\n"),
        ];
        SourceTreeTester::new(TestLeavesPythonVersionUntested, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("python3", "echo\n"))];
        let fail = vec![SourcePackageCase::from(tree(
            "python3-all",
            "py3versions -r\n",
        ))];
        SourcePackageTester::new(TestLeavesPythonVersionUntested, pass, fail).test_and_snapshot();
    }
}
