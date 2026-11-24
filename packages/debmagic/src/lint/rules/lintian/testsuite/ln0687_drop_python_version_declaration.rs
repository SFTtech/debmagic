use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::python::scan;
use super::sections::{field_line, tests_control_sections};

pub struct DropPythonVersionDeclaration;

declare_rule! {
    /// `X-Python3-Version` is declared but every test asks for the supported
    /// interpreters.
    ///
    /// Checked for each paragraph of `debian/tests/control`: it is reported
    /// when the source paragraph declares `X-Python3-Version` and every test
    /// file the paragraph names (regular files only) has a `py3versions` call
    /// with `-…s` or `--supported`; a paragraph without readable test files
    /// counts. Extra is empty. The Location is the `X-Python3-Version` line of
    /// `debian/control`. Applies to the Source tree and Source package.
    DropPythonVersionDeclaration,
    code = "LN0687",
    tag = "drop-python-version-declaration",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DropPythonVersionDeclaration {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        if !control.source().declares("X-Python3-Version") {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let count = sections
            .iter()
            .filter(|section| scan(files, section).all_tests_use_supported)
            .count();
        if count == 0 {
            return;
        }
        let line = files
            .get("debian/control")
            .and_then(|entry| files.read(entry).ok())
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .as_deref()
            .and_then(|text| field_line(text, "X-Python3-Version"));
        for _ in 0..count {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/control"),
                line,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(declared: bool) -> String {
        let field = if declared {
            "X-Python3-Version: >= 3.4\n"
        } else {
            ""
        };
        format!(
            "Source: example\nMaintainer: Example <ex@example.com>\n{field}\nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
        )
    }

    fn tree(declared: bool, script: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control(declared))),
            ("debian/tests/control", TestFile::new("Tests: t\n")),
            ("debian/tests/t", TestFile::new(script)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(false, "py3versions -s\n"),
            tree(true, "py3versions -r\n"),
            tree(true, "echo nothing\n"),
            vec![
                ("debian/control", TestFile::new(control(true))),
                ("debian/tests/control", TestFile::new("Tests: t u\n")),
                ("debian/tests/t", TestFile::new("py3versions -s\n")),
                ("debian/tests/u", TestFile::new("py3versions -v\n")),
            ],
            vec![("debian/control", TestFile::new(control(true)))],
        ];
        let fail = vec![
            tree(true, "py3versions -s\n"),
            tree(true, "echo $(py3versions -vs)\n"),
            vec![
                ("debian/control", TestFile::new(control(true))),
                (
                    "debian/tests/control",
                    TestFile::new("Test-Command: true\n"),
                ),
            ],
        ];
        SourceTreeTester::new(DropPythonVersionDeclaration, pass, fail).test_and_snapshot();
    }
}
