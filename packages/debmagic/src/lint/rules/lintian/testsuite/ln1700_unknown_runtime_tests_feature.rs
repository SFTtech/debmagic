use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::control::TEST_SPLIT;
use super::sections::{control_location, tests_control_sections};

pub struct UnknownRuntimeTestsFeature;

declare_rule! {
    /// A paragraph of `debian/tests/control` lists a `Features` value that
    /// lintian does not know. Allowed, but the value is ignored, so it may
    /// indicate an error.
    ///
    /// lintian knows no feature names, so every word is reported except
    /// `test-name=` followed by a non-empty name. `Features` is split on commas
    /// and whitespace, and each word is compared exactly. Extra is the word.
    /// The Location is the `Features` line. Reported at pedantic severity.
    /// Applies to the Source tree and Source package.
    UnknownRuntimeTestsFeature,
    code = "LN1700",
    tag = "unknown-runtime-tests-feature",
    default_selected = false,
    experimental = false,
}

fn is_known(feature: &str) -> bool {
    feature
        .strip_prefix("test-name=")
        .is_some_and(|name| !name.is_empty())
}

impl SourceRule for UnknownRuntimeTestsFeature {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let mut hits = Vec::new();
        for section in &sections {
            let line = section.line("Features");
            let value = section.unfolded_value("Features");
            for feature in TEST_SPLIT.split(value.trim()).map(str::trim) {
                if !feature.is_empty() && !is_known(feature) {
                    hits.push((feature.to_string(), line));
                }
            }
        }
        for (feature, line) in hits {
            ctx.diagnostic(
                Diagnostic::warn(feature)
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
            tree("Tests: smoke\n"),
            tree("Tests: smoke\nFeatures: test-name=smoke\n"),
            tree("Tests: smoke\nFeatures:\n"),
            tree("Tests: smoke\nFeatures: test-name=a, test-name=b\n"),
        ];
        let fail = vec![
            tree("Tests: smoke\nFeatures: unknown\n"),
            tree("Tests: smoke\nFeatures: test-name=, test-name\n"),
            tree("Tests: a\nFeatures: test-name=ok,\n bogus\n\nTests: b\nFeatures: x y\n"),
        ];
        SourceTreeTester::new(UnknownRuntimeTestsFeature, pass, fail).test_and_snapshot();
    }
}
