use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::sections::{control_location, tests_control_sections};

pub struct UnknownRuntimeTestsField;

const KNOWN_FIELDS: [&str; 8] = [
    "tests",
    "restrictions",
    "features",
    "depends",
    "tests-directory",
    "test-command",
    "architecture",
    "classes",
];

declare_rule! {
    /// A paragraph of `debian/tests/control` has a field name that is not one
    /// of lintian's known fields: `Tests`, `Restrictions`, `Features`,
    /// `Depends`, `Tests-Directory`, `Test-Command`, `Architecture` and
    /// `Classes`.
    ///
    /// Names are compared case-insensitively. Extra is the field name as
    /// written, and the Location is that field's line. Reported at pedantic
    /// severity. Applies to the Source tree and Source package.
    UnknownRuntimeTestsField,
    code = "LN1701",
    tag = "unknown-runtime-tests-field",
    default_selected = false,
    experimental = false,
}

impl SourceRule for UnknownRuntimeTestsField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let mut hits = Vec::new();
        for section in &sections {
            for (name, line) in section.field_names() {
                if !KNOWN_FIELDS.contains(&name.to_ascii_lowercase().as_str()) {
                    hits.push((name.to_string(), line));
                }
            }
        }
        for (name, line) in hits {
            ctx.diagnostic(
                Diagnostic::warn(name)
                    .with_severity(Severity::Pedantic)
                    .with_location(control_location(Some(line))),
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
            tree("Tests: smoke\nrestrictions: allow-stderr\nTESTS-DIRECTORY: x\n"),
            tree("Test-Command: /bin/true\nDepends: @\nArchitecture: any\nClasses: x\n"),
            tree("Tests: smoke\n# Comment: ignored\n"),
        ];
        let fail = vec![
            tree("Comment: hello\nTests: smoke\n"),
            tree("Tests: smoke\nFeatures: x\n\nTests: b\nMaintainer: nope\nX-Foo: 1\n"),
        ];
        SourceTreeTester::new(UnknownRuntimeTestsField, pass, fail).test_and_snapshot();
    }
}
