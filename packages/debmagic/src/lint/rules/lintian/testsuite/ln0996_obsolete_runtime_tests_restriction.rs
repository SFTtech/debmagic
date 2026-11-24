use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::control::TEST_SPLIT;
use super::sections::{control_location, tests_control_sections};

pub struct ObsoleteRuntimeTestsRestriction;

declare_rule! {
    /// A paragraph of `debian/tests/control` lists an obsolete `Restrictions`
    /// value.
    ///
    /// Obsolete values are lintian's `testsuite/known-obsolete-restrictions`
    /// (currently `needs-recommends`). `Restrictions` is split on commas and
    /// whitespace, and each word is compared exactly. One finding per matching
    /// word. Extra is the word. The Location is the `Restrictions` line. Applies
    /// to the Source tree and Source package.
    ObsoleteRuntimeTestsRestriction,
    code = "LN0996",
    tag = "obsolete-runtime-tests-restriction",
    default_selected = true,
    experimental = false,
}

static OBSOLETE: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!(
        "../../../data/testsuite/known-obsolete-restrictions"
    ))
    .collect()
});

fn restrictions(value: &str) -> Vec<String> {
    TEST_SPLIT
        .split(value.trim())
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

impl SourceRule for ObsoleteRuntimeTestsRestriction {
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
            for word in restrictions(&section.unfolded_value("Restrictions")) {
                if OBSOLETE.contains(&word.as_str()) {
                    hits.push((word, line));
                }
            }
        }
        for (word, line) in hits {
            ctx.diagnostic(Diagnostic::warn(word).with_location(control_location(line)));
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
            tree("Tests: smoke\nRestrictions: needs-recommendsx\n"),
            tree("Tests: smoke\n"),
        ];
        let fail = vec![
            tree("Tests: smoke\nRestrictions: needs-recommends\n"),
            tree("Tests: a\n\nTests: b\nRestrictions: allow-stderr,\n needs-recommends\n"),
        ];
        SourceTreeTester::new(ObsoleteRuntimeTestsRestriction, pass, fail).test_and_snapshot();
    }
}
