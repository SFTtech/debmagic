use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::control;

pub struct UsesDeprecatedAdttmp;

declare_rule! {
    /// A runtime test script named by `Tests` in `debian/tests/control` uses
    /// the deprecated `ADTTMP` variable instead of `AUTOPKGTEST_TMP`.
    ///
    /// Reads `Tests` (and `Tests-Directory`, default `debian/tests`) from
    /// `debian/tests/control`, as the other testsuite Rules do. Each test file
    /// is followed through symbolic links inside the tree and must then be a
    /// regular file or hardlink; any other is skipped. Every line containing
    /// `ADTTMP` is reported, so a file can report more than once. Extra is
    /// empty and Location is the test file with the line. Skipped when there
    /// is no `debian/tests/control`, or it is not valid UTF-8.
    UsesDeprecatedAdttmp,
    code = "LN1142",
    tag = "uses-deprecated-adttmp",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UsesDeprecatedAdttmp {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(control_text) = control::read_tests_control(files) else {
            return;
        };
        let mut hits = Vec::new();
        for path in control::test_paths(&control_text) {
            let Some((location, text)) = control::read_regular(files, &path) else {
                continue;
            };
            for (index, line) in text.lines().enumerate() {
                if line.contains("ADTTMP") {
                    let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
                    hits.push((location.clone(), line_number));
                }
            }
        }
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn(String::new()).with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\nMaintainer: Example <ex@example.com>\nStandards-Version: 4.7.2\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n";

    const DSC: &str = "Format: 3.0 (native)\n\
         Source: example\n\
         Version: 1.0\n\
         Maintainer: Example <ex@example.com>\n\
         Checksums-Sha1:\n da39a3ee5e6b4b0d3255bfef95601890afd80709 0 empty\n\
         Checksums-Sha256:\n e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 0 empty\n\
         Files:\n d41d8cd98f00b204e9800998ecf8427e 0 empty\n";

    fn dsc() -> Dsc {
        Dsc::parse(DSC).expect("Dsc fixture")
    }

    #[test]
    fn source() {
        let pass = vec![
            // The variable is replaced.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                (
                    "debian/tests/control",
                    TestFile::new("Tests: run-unit-test\n"),
                ),
                (
                    "debian/tests/run-unit-test",
                    TestFile::new("#!/bin/sh\necho $AUTOPKGTEST_TMP\n"),
                ),
            ],
            // A test file that no Tests field names is not read.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                (
                    "debian/tests/control",
                    TestFile::new("Tests: run-unit-test\n"),
                ),
                ("debian/tests/other", TestFile::new("echo $ADTTMP\n")),
            ],
            // A test file that is a directory is skipped.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                (
                    "debian/tests/control",
                    TestFile::new("Tests: run-unit-test\n"),
                ),
                ("debian/tests/run-unit-test", TestFile::directory()),
            ],
            // No debian/tests/control: nothing is read.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                (
                    "debian/tests/run-unit-test",
                    TestFile::new("echo $ADTTMP\n"),
                ),
            ],
        ];
        let fail = vec![
            // The file is named by Tests, the line is reported.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                (
                    "debian/tests/control",
                    TestFile::new("Tests: run-unit-test\n"),
                ),
                (
                    "debian/tests/run-unit-test",
                    TestFile::new("#!/bin/sh\nset -e\ncp -r . \"$ADTTMP\"\n"),
                ),
            ],
            // Every line counts, and each test in a section is read.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                (
                    "debian/tests/control",
                    TestFile::new("Tests: one two\n\nTests: three\nTests-Directory: tests\n"),
                ),
                (
                    "debian/tests/one",
                    TestFile::new("echo $ADTTMP\necho $ADTTMP\n"),
                ),
                ("debian/tests/two", TestFile::new("echo ok\n")),
                ("debian/tests/three", TestFile::new("echo ok\n")),
                ("tests/three", TestFile::new("echo $ADTTMP\n")),
            ],
            // A symbolic link inside the tree is followed; the reported path is the target.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/tests/control", TestFile::new("Tests: link\n")),
                ("debian/tests/link", TestFile::symlink("run-unit-test")),
                (
                    "debian/tests/run-unit-test",
                    TestFile::new("echo $ADTTMP\n"),
                ),
            ],
            // Tests-Directory: . names paths from the top of the tree.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                (
                    "debian/tests/control",
                    TestFile::new("Tests-Directory: .\nTests: run-unit-test\n"),
                ),
                ("run-unit-test", TestFile::new("echo $ADTTMP\n")),
            ],
        ];
        SourceTreeTester::new(UsesDeprecatedAdttmp, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            SourcePackageCase::from(vec![
                ("debian/control", TestFile::new(CONTROL)),
                (
                    "debian/tests/control",
                    TestFile::new("Tests: run-unit-test\n"),
                ),
                (
                    "debian/tests/run-unit-test",
                    TestFile::new("echo $AUTOPKGTEST_TMP\n"),
                ),
            ])
            .dsc(dsc()),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![
                ("debian/control", TestFile::new(CONTROL)),
                (
                    "debian/tests/control",
                    TestFile::new("Tests: run-unit-test\n"),
                ),
                (
                    "debian/tests/run-unit-test",
                    TestFile::new("echo $ADTTMP\n"),
                ),
            ])
            .dsc(dsc()),
        ];
        SourcePackageTester::new(UsesDeprecatedAdttmp, pass, fail).test_and_snapshot();
    }
}
