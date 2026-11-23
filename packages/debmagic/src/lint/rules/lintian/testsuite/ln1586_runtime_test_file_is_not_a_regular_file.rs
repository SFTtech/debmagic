use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{Severity, SourceRule};

use super::sections::{control_location, resolve, tests_control_sections};

pub struct RuntimeTestFileIsNotARegularFile;

declare_rule! {
    /// A test named in `Tests` exists in the source tree but is not a regular
    /// file, or a symbolic link that resolves to one.
    ///
    /// Names are the unique entries of `Tests`, prefixed with `Tests-Directory`
    /// (default `debian/tests`; `.` adds no prefix). The path is followed
    /// through symbolic links inside the tree, and what it ends in has to be a
    /// regular file or a hardlink (lintian's `is_open_ok`): a directory, a
    /// named pipe or a device is reported, and so is a symlink to one. A path
    /// that does not resolve is `missing-runtime-test-file`'s, not reported
    /// here. Extra is the path. The Location is the `Tests` line. Applies to
    /// the Source tree and Source package.
    RuntimeTestFileIsNotARegularFile,
    code = "LN1586",
    tag = "runtime-test-file-is-not-a-regular-file",
    default_selected = false,
    experimental = false,
}

impl SourceRule for RuntimeTestFileIsNotARegularFile {
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
                let Some(entry) = resolve(files, &path) else {
                    continue;
                };
                if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                    hits.push((path, line));
                }
            }
        }
        for (path, line) in hits {
            ctx.diagnostic(
                Diagnostic::warn(path)
                    .with_severity(Severity::Info)
                    .with_location(control_location(line)),
            );
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
            tree("Tests: one\n", vec![("debian/tests/one", script())]),
            // A missing test is `missing-runtime-test-file`'s.
            tree("Tests: gone\n", vec![]),
            tree(
                "Tests: alias\n",
                vec![
                    ("debian/tests/real", script()),
                    ("debian/tests/alias", TestFile::symlink("real")),
                ],
            ),
            tree(
                "Tests: copy\n",
                vec![
                    ("debian/tests/real", script()),
                    (
                        "debian/tests/copy",
                        script().hardlink_to("debian/tests/real"),
                    ),
                ],
            ),
        ];
        let fail = vec![
            tree(
                "Tests: pipe\n",
                vec![("debian/tests/pipe", TestFile::fifo())],
            ),
            tree(
                "Tests: link\n",
                vec![
                    ("debian/tests/pipe", TestFile::fifo()),
                    ("debian/tests/link", TestFile::symlink("pipe")),
                ],
            ),
            tree(
                "Tests: sub\n",
                vec![("debian/tests/sub", TestFile::directory())],
            ),
            tree(
                "Tests: pipe pipe other\n\nTests: pipe\n",
                vec![
                    ("debian/tests/pipe", TestFile::fifo()),
                    ("debian/tests/other", TestFile::directory()),
                ],
            ),
        ];
        SourceTreeTester::new(RuntimeTestFileIsNotARegularFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "Tests: one\n",
            vec![("debian/tests/one", script())],
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "Tests: one\n",
            vec![("debian/tests/one", TestFile::fifo())],
        ))];
        SourcePackageTester::new(RuntimeTestFileIsNotARegularFile, pass, fail).test_and_snapshot();
    }
}
