use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::control;

pub struct RuntimeTestFileUsesInstalledPythonVersions;

declare_rule! {
    /// An autopkgtest script asks `py3versions` for the installed interpreters.
    ///
    /// Reads `Tests` (and `Tests-Directory`, default `debian/tests`) from
    /// `debian/tests/control`. A line matching `py3versions` plus short or
    /// long options is reported when an option is `-…i` or `--installed`.
    /// Extra is the command and those options. The Location is that line.
    RuntimeTestFileUsesInstalledPythonVersions,
    code = "LN0377",
    tag = "runtime-test-file-uses-installed-python-versions",
    default_selected = true,
    experimental = false,
}

static PY3VERSIONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(py3versions)((?:\s+--?\w+)*)").expect("valid regex"));
static INSTALLED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s(?:-\w*i|--installed)").expect("valid regex"));

impl SourceRule for RuntimeTestFileUsesInstalledPythonVersions {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(control_text) = control::read_tests_control(files) else {
            return;
        };
        let paths = control::test_paths(&control_text);
        let mut hits = Vec::new();
        for path in paths {
            let Some((location, text)) = control::read_regular(files, &path) else {
                continue;
            };
            for (index, line) in text.lines().enumerate() {
                let Some(captures) = PY3VERSIONS.captures(line) else {
                    continue;
                };
                let options = captures.get(2).map(|m| m.as_str()).unwrap_or("");
                if !INSTALLED.is_match(options) {
                    continue;
                }
                let command = format!(
                    "{}{options}",
                    captures.get(1).map(|m| m.as_str()).unwrap_or("")
                );
                let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
                hits.push((location.clone(), line_number, command));
            }
        }
        for (path, line, command) in hits {
            ctx.diagnostic(Diagnostic::warn(command).with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\nMaintainer: Example <ex@example.com>\nStandards-Version: 4.7.2\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n";

    fn tree(extra: &[(&str, &str)]) -> Vec<(String, TestFile)> {
        let mut files = vec![("debian/control".to_string(), TestFile::from(CONTROL))];
        for (path, content) in extra {
            files.push(((*path).to_string(), TestFile::new(*content)));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[("debian/tests/control", "Tests: test-1\n")]),
            tree(&[
                ("debian/tests/control", "Tests: test-1\n"),
                (
                    "debian/tests/test-1",
                    "#!/bin/sh\npy3versions -s\npy3versions --supported\n",
                ),
            ]),
            tree(&[
                ("debian/tests/control", "Test-Command: true\n"),
                ("debian/tests/test-1", "py3versions -i\n"),
            ]),
        ];
        let fail = vec![
            tree(&[
                ("debian/tests/control", "Tests: test-1\n"),
                (
                    "debian/tests/test-1",
                    "#!/bin/sh\npy3versions -i\npy3versions -vi\npy3versions --installed\npy3versions --verbose --installed\npy3versions -s\n",
                ),
            ]),
            tree(&[
                ("debian/tests/control", "Tests: check\nTests-Directory: .\n"),
                ("check", "py3versions -is\n"),
            ]),
        ];
        SourceTreeTester::new(RuntimeTestFileUsesInstalledPythonVersions, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let files = tree(&[
            ("debian/tests/control", "Tests: test-1\n"),
            ("debian/tests/test-1", "py3versions -i\n"),
        ]);
        let pass = vec![SourcePackageCase::from(tree(&[]))];
        let fail = vec![SourcePackageCase::from(files)];
        SourcePackageTester::new(RuntimeTestFileUsesInstalledPythonVersions, pass, fail)
            .test_and_snapshot();
    }
}
