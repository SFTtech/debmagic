use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::sections::{control_location, tests_control_sections};

pub struct NoTests;

declare_rule! {
    /// A `debian/tests/control` paragraph declares neither `Tests` nor
    /// `Test-Command`, so the autopkgtest defines no test.
    ///
    /// Each paragraph is checked on its own, in file order; a paragraph that
    /// declares either field (in any letter case) is fine. Extra is empty, so
    /// the Location is compared in Parity: it is the first line of the
    /// paragraph, after any leading comment lines. The Location is
    /// `debian/tests/control`. A missing, non-regular, non-UTF-8 or unparsable
    /// file reports nothing, as the Check dies or returns there in lintian.
    /// Applies to the Source tree and Source package.
    NoTests,
    code = "LN1781",
    tag = "no-tests",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NoTests {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let lines: Vec<u32> = sections
            .iter()
            .filter(|section| !section.declares("Tests") && !section.declares("Test-Command"))
            .map(|section| section.start())
            .collect();
        for line in lines {
            ctx.diagnostic(Diagnostic::warn("").with_location(control_location(Some(line))));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/tests/control", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no tests"))],
            tree("Tests: smoke\n"),
            tree("Test-Command: /bin/true\n"),
            tree("tests: smoke\nDepends: @\n"),
            tree("Tests: a\n\nTest-Command: b\n"),
            tree("Tests: a\n\n# Depends: @\nTEST-COMMAND: b\n"),
            tree("Tests: a\n\nTests: b\n"),
            // A symbolic link is not a regular file: lintian dies there before
            // any paragraph is checked.
            vec![
                ("debian/tests", TestFile::directory()),
                ("debian/tests/control", TestFile::symlink("real")),
                ("debian/tests/real", TestFile::new("Depends: @\n")),
            ],
            // Not UTF-8: the Check returns before reading paragraphs.
            vec![(
                "debian/tests/control",
                TestFile::from_bytes(b"Depends: \xff\n".to_vec()),
            )],
        ];
        let fail = vec![
            tree("Depends: @\n"),
            tree("# comment\nDepends: @\n"),
            tree("Tests: smoke\n\nDepends: @\n"),
            tree("Tests: smoke\n\n# note\nDepends: @\n\nRestrictions: needs-root\n"),
            tree("Tests: a\n\n\nDepends: @\n\nDepends: @\n"),
        ];
        SourceTreeTester::new(NoTests, pass, fail).test_and_snapshot();
    }
}
