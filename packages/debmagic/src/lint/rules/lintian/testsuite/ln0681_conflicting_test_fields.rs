use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::sections::{control_location, tests_control_sections};

pub struct ConflictingTestFields;

declare_rule! {
    /// A `debian/tests/control` paragraph has both `Tests` and `Test-Command`.
    ///
    /// Extra is `Tests Test-Command`. The Location is the first line of the
    /// paragraph. Applies to the Source tree and Source package.
    ConflictingTestFields,
    code = "LN0681",
    tag = "conflicting-test-fields",
    default_selected = true,
    experimental = false,
}

impl SourceRule for ConflictingTestFields {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let lines: Vec<u32> = sections
            .iter()
            .filter(|section| section.declares("Tests") && section.declares("Test-Command"))
            .map(|section| section.start())
            .collect();
        for line in lines {
            ctx.diagnostic(
                Diagnostic::warn("Tests Test-Command").with_location(control_location(Some(line))),
            );
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
            tree("Tests: smoke\n\nTest-Command: true\n"),
            tree("Tests: smoke\n# Test-Command: true\n"),
        ];
        let fail = vec![
            tree("Tests: smoke\nTest-Command: true\n"),
            tree("Tests: a\n\n# note\nTest-Command: true\ntests: b\n\nTests: c\nTest-Command: x\n"),
        ];
        SourceTreeTester::new(ConflictingTestFields, pass, fail).test_and_snapshot();
    }
}
