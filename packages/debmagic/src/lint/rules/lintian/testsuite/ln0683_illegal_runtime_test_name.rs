use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::sections::{control_location, tests_control_sections};

pub struct IllegalRuntimeTestName;

declare_rule! {
    /// A name in `Tests` has a character other than letters, digits, `+`, `-`,
    /// `.` and `/`.
    ///
    /// `Tests` is split on commas and whitespace and repeated names are
    /// dropped. Extra is the name. The Location is the `Tests` line. Applies to
    /// the Source tree and Source package.
    IllegalRuntimeTestName,
    code = "LN0683",
    tag = "illegal-runtime-test-name",
    default_selected = true,
    experimental = false,
}

fn is_legal(name: &str) -> bool {
    name.chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '+' | '-' | '.' | '/'))
}

impl SourceRule for IllegalRuntimeTestName {
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
            for name in section.unique_tests() {
                if !is_legal(&name) {
                    hits.push((name, line));
                }
            }
        }
        for (name, line) in hits {
            ctx.diagnostic(Diagnostic::warn(name).with_location(control_location(line)));
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
            tree("Tests: test-1 sub/test.2 c++ \u{e4}b\n"),
            tree("Test-Command: under_score\n"),
        ];
        let fail = vec![
            tree("Tests: fifo missing-test under_score\n"),
            tree("Tests: a_b, a_b, c@d\n\nTests: x*\n"),
        ];
        SourceTreeTester::new(IllegalRuntimeTestName, pass, fail).test_and_snapshot();
    }
}
