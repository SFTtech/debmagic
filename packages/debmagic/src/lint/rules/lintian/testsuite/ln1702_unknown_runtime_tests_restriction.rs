use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::control::TEST_SPLIT;
use super::sections::{control_location, tests_control_sections};

pub struct UnknownRuntimeTestsRestriction;

declare_rule! {
    /// A paragraph of `debian/tests/control` lists a `Restrictions` value that
    /// is not in lintian's `testsuite/known-restrictions`.
    ///
    /// `Restrictions` is split on commas and whitespace, and each word is
    /// compared exactly. One finding per unknown word. Extra is the word. The
    /// Location is the `Restrictions` line. Reported at pedantic severity.
    /// Applies to the Source tree and Source package.
    UnknownRuntimeTestsRestriction,
    code = "LN1702",
    tag = "unknown-runtime-tests-restriction",
    default_selected = false,
    experimental = false,
}

static KNOWN: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/testsuite/known-restrictions")).collect()
});

impl SourceRule for UnknownRuntimeTestsRestriction {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let mut hits = Vec::new();
        for section in &sections {
            let line = section.line("Restrictions");
            let value = section.unfolded_value("Restrictions");
            for word in TEST_SPLIT.split(value.trim()).map(str::trim) {
                if !word.is_empty() && !KNOWN.contains(&word) {
                    hits.push((word.to_string(), line));
                }
            }
        }
        for (word, line) in hits {
            ctx.diagnostic(
                Diagnostic::warn(word)
                    .with_severity(Severity::Pedantic)
                    .with_location(control_location(line)),
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
            tree("Tests: smoke\nRestrictions: allow-stderr\n"),
            tree("Tests: smoke\nRestrictions: needs-root, rw-build-tree\n"),
            tree("Tests: smoke\nRestrictions: needs-recommends, skippable\n"),
            tree("Tests: smoke\n"),
        ];
        let fail = vec![
            tree("Tests: smoke\nRestrictions: unknownrestriction\n"),
            tree(
                "Tests: a\nRestrictions: Allow-Stderr, needs-root\n\nTests: b\nRestrictions:\n rw-build-tree\n bogus\n",
            ),
        ];
        SourceTreeTester::new(UnknownRuntimeTestsRestriction, pass, fail).test_and_snapshot();
    }
}
