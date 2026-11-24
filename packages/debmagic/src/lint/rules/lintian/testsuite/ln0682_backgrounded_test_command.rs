use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::sections::{control_location, tests_control_sections};

pub struct BackgroundedTestCommand;

declare_rule! {
    /// A `Test-Command` ends with `&`.
    ///
    /// The unfolded command, trimmed on both sides, is tested for a trailing
    /// `&` (a trailing `&&` counts too, as in lintian). Extra is the trimmed
    /// command. The Location is the `Test-Command` line. Applies to the Source
    /// tree and Source package.
    BackgroundedTestCommand,
    code = "LN0682",
    tag = "backgrounded-test-command",
    default_selected = true,
    experimental = false,
}

impl SourceRule for BackgroundedTestCommand {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let hits: Vec<(String, Option<u32>)> = sections
            .iter()
            .filter_map(|section| {
                let command = section.unfolded_value("Test-Command").trim().to_string();
                command
                    .ends_with('&')
                    .then(|| (command, section.line("Test-Command")))
            })
            .collect();
        for (command, line) in hits {
            ctx.diagnostic(Diagnostic::error(command).with_location(control_location(line)));
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
            tree("Test-Command: /bin/true\n"),
            tree("Test-Command: sleep 1 & wait\n"),
            tree("Tests: smoke &\n"),
        ];
        let fail = vec![
            tree("Test-Command: /bin/true &\nDepends: @\n"),
            tree("Test-Command:   server --run&   \n"),
            tree("Test-Command: ok\n\nTest-Command: first\n second &\n"),
        ];
        SourceTreeTester::new(BackgroundedTestCommand, pass, fail).test_and_snapshot();
    }
}
