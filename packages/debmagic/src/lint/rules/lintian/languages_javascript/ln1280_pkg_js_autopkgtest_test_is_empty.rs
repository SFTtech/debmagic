use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct PkgJsAutopkgtestTestIsEmpty;

declare_rule! {
    /// The `debian/tests/pkg-js/test` file of a Source whose `Testsuite` field
    /// lists `autopkgtest-pkg-nodejs` has no content, so the autopkgtest
    /// would report success without testing anything.
    ///
    /// The file counts as empty unless some line has a word character (ASCII
    /// letter, digit or `_`) before its first `#`. A missing file is not
    /// reported here. A directory at that path counts as empty, as it does in
    /// lintian. The Location is the file, without a line.
    PkgJsAutopkgtestTestIsEmpty,
    code = "LN1280",
    tag = "pkg-js-autopkgtest-test-is-empty",
    default_selected = true,
    experimental = false,
}

const TEST_FILE: &str = "debian/tests/pkg-js/test";

impl SourceRule for PkgJsAutopkgtestTestIsEmpty {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let uses_nodejs_testsuite = ctx
            .debian_control()
            .and_then(|control| control.source().unfolded_value("Testsuite"))
            .is_some_and(|value| {
                value
                    .split(',')
                    .any(|suite| suite.trim() == "autopkgtest-pkg-nodejs")
            });
        if !uses_nodejs_testsuite {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = files.get(TEST_FILE) else {
            return;
        };
        let bytes = if entry.kind() == EntryKind::Directory {
            Vec::new()
        } else {
            let Ok(bytes) = files.read(entry) else {
                return;
            };
            bytes
        };
        if !has_content(&bytes) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: TEST_FILE.into(),
                line: None,
                column: None,
            }));
        }
    }
}

/// Whether some line has a word character before its first `#`.
fn has_content(bytes: &[u8]) -> bool {
    bytes.split(|&byte| byte == b'\n').any(|line| {
        line.split(|&byte| byte == b'#')
            .next()
            .is_some_and(|code| code.iter().any(|&b| b.is_ascii_alphanumeric() || b == b'_'))
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\nTestsuite: autopkgtest-pkg-nodejs\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n";

    fn package(test: Option<TestFile>) -> TestCase {
        let mut files = vec![("debian/control", TestFile::new(CONTROL))];
        if let Some(test) = test {
            files.push((TEST_FILE, test));
        }
        TestCase::source_tree(files)
    }

    #[test]
    fn source() {
        let pass = vec![
            package(Some(TestFile::new("#!/bin/sh\nnode index.js\n"))),
            package(Some(TestFile::new("node index.js # run it\n"))),
            package(Some(TestFile::new("_\n"))),
            // A missing file is not this Tag.
            package(None),
            // Without the nodejs Testsuite the file is not checked.
            TestCase::source_tree([
                (
                    "debian/control",
                    TestFile::new(
                        "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
                    ),
                ),
                (TEST_FILE, TestFile::new("")),
            ]),
        ];
        let fail = vec![
            package(Some(TestFile::new(""))),
            package(Some(TestFile::new("\n\n  \n"))),
            package(Some(TestFile::new("# Does nothing\n\n# still nothing\n"))),
            package(Some(TestFile::new("  # indented comment\n"))),
            package(Some(TestFile::directory())),
        ];
        SourceTreeTester::new(PkgJsAutopkgtestTestIsEmpty, pass, fail).test_and_snapshot();
    }
}
