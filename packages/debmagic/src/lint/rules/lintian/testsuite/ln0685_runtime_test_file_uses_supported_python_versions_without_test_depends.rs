use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::python::{depends_on_python3_all, scan};
use super::sections::tests_control_sections;

pub struct RuntimeTestFileUsesSupportedPythonVersionsWithoutTestDepends;

declare_rule! {
    /// A test asks `py3versions` for the supported interpreters without
    /// depending on `python3-all`.
    ///
    /// For each paragraph of `debian/tests/control`, a `py3versions` line with
    /// `-…s` or `--supported` among its options is reported when the paragraph's
    /// `Depends` (with `@builddeps@` and `@recommends@` expanded) does not imply
    /// `python3-all`, `python3-all-dev` or `python3-all-dbg`. Extra is the
    /// command and its options. The Location is that line. Applies to the Source
    /// tree and Source package.
    RuntimeTestFileUsesSupportedPythonVersionsWithoutTestDepends,
    code = "LN0685",
    tag = "runtime-test-file-uses-supported-python-versions-without-test-depends",
    default_selected = true,
    experimental = false,
}

impl SourceRule for RuntimeTestFileUsesSupportedPythonVersionsWithoutTestDepends {
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
        let mut hits: Vec<(String, Location)> = Vec::new();
        for section in &sections {
            if depends_on_python3_all(section, &control) {
                continue;
            }
            hits.extend(
                scan(files, section)
                    .calls
                    .into_iter()
                    .filter(|call| call.asks_supported())
                    .map(|call| (call.command, call.location)),
            );
        }
        for (command, location) in hits {
            ctx.diagnostic(Diagnostic::warn(command).with_location(location));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\nMaintainer: Example <ex@example.com>\nBuild-Depends: debhelper-compat (= 13), python3-all\n\nPackage: example\nArchitecture: all\nRecommends: python3-all-dev\nDescription: example\n extra\n";

    fn tree(tests: &str, script: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/tests/control", TestFile::new(tests)),
            ("debian/tests/t", TestFile::new(script)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("Tests: t\nDepends: python3-all\n", "py3versions -s\n"),
            tree(
                "Tests: t\nDepends: python3-all-dev:any, @\n",
                "py3versions --supported\n",
            ),
            tree("Tests: t\nDepends: @builddeps@\n", "py3versions -vs\n"),
            tree("Tests: t\nDepends: @recommends@\n", "py3versions -s\n"),
            tree(
                "Tests: t\n",
                "py3versions -i\npy3versions -r\npy3versions -v\n",
            ),
        ];
        let fail = vec![
            tree(
                "Tests: t\n",
                "py3versions -s\npy3versions -vs\npy3versions --supported\npy3versions --verbose --supported\npy3versions -is\n",
            ),
            tree("Tests: t\nDepends: @\n", "echo $(py3versions -s)\n"),
            tree("Tests: t\nDepends: python3\n", "py3versions -s\n"),
        ];
        SourceTreeTester::new(
            RuntimeTestFileUsesSupportedPythonVersionsWithoutTestDepends,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
