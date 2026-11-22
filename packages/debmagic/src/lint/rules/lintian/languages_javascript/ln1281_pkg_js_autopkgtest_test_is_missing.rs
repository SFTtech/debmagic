use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct PkgJsAutopkgtestTestIsMissing;

declare_rule! {
    /// The `Testsuite` field of a Source lists `autopkgtest-pkg-nodejs`, but
    /// `debian/tests/pkg-js/test` does not exist. Only `node require` of the
    /// module is then run as the autopkgtest, which may not really test it.
    ///
    /// A path counts as present when any Entry sits at it (a symlink or a
    /// directory too). The diagnostic has no Location, as in lintian.
    PkgJsAutopkgtestTestIsMissing,
    code = "LN1281",
    tag = "pkg-js-autopkgtest-test-is-missing",
    default_selected = true,
    experimental = false,
}

const TEST_FILE: &str = "debian/tests/pkg-js/test";

impl SourceRule for PkgJsAutopkgtestTestIsMissing {
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
        if files.get(TEST_FILE).is_none() {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
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
            package(Some(TestFile::new("node index.js\n"))),
            // An empty file is present; LN1280 reports it.
            package(Some(TestFile::new(""))),
            // A directory at the path is present.
            package(Some(TestFile::directory())),
            // Without the nodejs Testsuite the file is not checked.
            TestCase::source_tree([(
                "debian/control",
                TestFile::new(
                    "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
                ),
            )]),
        ];
        let fail = vec![
            package(None),
            // Only the sibling files exist.
            TestCase::source_tree([
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/tests/pkg-js/files", TestFile::new("index.js\n")),
            ]),
        ];
        SourceTreeTester::new(PkgJsAutopkgtestTestIsMissing, pass, fail).test_and_snapshot();
    }
}
