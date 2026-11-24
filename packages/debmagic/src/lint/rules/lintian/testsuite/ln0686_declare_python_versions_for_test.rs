use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::python::scan;
use super::sections::tests_control_sections;

pub struct DeclarePythonVersionsForTest;

declare_rule! {
    /// A test asks `py3versions` for the requested interpreters while the
    /// source has no `X-Python3-Version`.
    ///
    /// A `py3versions` line with `-…r` or `--requested` among its options is
    /// reported when the source paragraph of `debian/control` does not declare
    /// `X-Python3-Version`. Extra is the command and its options. The Location
    /// is that line. Applies to the Source tree and Source package.
    DeclarePythonVersionsForTest,
    code = "LN0686",
    tag = "declare-python-versions-for-test",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DeclarePythonVersionsForTest {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        if control.source().declares("X-Python3-Version") {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let mut hits: Vec<(String, Location)> = Vec::new();
        for section in &sections {
            hits.extend(
                scan(files, section)
                    .calls
                    .into_iter()
                    .filter(|call| call.asks_requested())
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

    fn control(extra: &str) -> String {
        format!(
            "Source: example\nMaintainer: Example <ex@example.com>\n{extra}\nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
        )
    }

    fn tree(extra: &str, script: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control(extra))),
            ("debian/tests/control", TestFile::new("Tests: t\n")),
            ("debian/tests/t", TestFile::new(script)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("X-Python3-Version: >= 3.4\n", "py3versions -r\n"),
            tree("", "py3versions -s\npy3versions -i\npy3versions -v\n"),
            vec![("debian/control", TestFile::new(control("")))],
        ];
        let fail = vec![tree(
            "",
            "py3versions -r\npy3versions -vr\npy3versions --requested\npy3versions --verbose --requested\n",
        )];
        SourceTreeTester::new(DeclarePythonVersionsForTest, pass, fail).test_and_snapshot();
    }
}
