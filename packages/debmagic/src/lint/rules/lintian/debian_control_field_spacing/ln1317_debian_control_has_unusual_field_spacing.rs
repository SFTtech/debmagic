use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct DebianControlHasUnusualFieldSpacing;

declare_rule! {
    /// A field in `debian/control` has other than one space after its colon.
    ///
    /// Reads `debian/control` as written, line by line. Trailing whitespace is
    /// dropped and comment lines are skipped. A line that starts with a field
    /// name and colon is reported unless one space follows the colon and then a
    /// non-space character, or the colon ends the line. Extra is the field name.
    /// Pedantic, selected only when asked for. Invalid UTF-8 reports nothing,
    /// as lintian's `decoded_utf8` is empty there.
    DebianControlHasUnusualFieldSpacing,
    code = "LN1317",
    tag = "debian-control-has-unusual-field-spacing",
    default_selected = false,
    experimental = false,
}

/// Field name and colon at the start of a line (lintian's `^ (\S+) : `).
static FIELD_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\S+):").expect("valid regex"));
/// The spacing lintian accepts: one space and a value, or nothing after the
/// colon.
static USUAL_SPACING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\S+: \S|^\S+:$").expect("valid regex"));

impl SourceRule for DebianControlHasUnusualFieldSpacing {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = files.get("debian/control") else {
            return;
        };
        let Ok(bytes) = files.read(entry) else {
            return;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            return;
        };

        let mut reports = Vec::new();
        for (index, raw) in text.split('\n').enumerate() {
            let line = raw.trim_end();
            if line.starts_with('#') {
                continue;
            }
            let Some(captures) = FIELD_START.captures(line) else {
                continue;
            };
            if USUAL_SPACING.is_match(line) {
                continue;
            }
            reports.push((captures[1].to_string(), index as u32 + 1));
        }

        for (field, line) in reports {
            ctx.diagnostic(
                Diagnostic::warn(field)
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path: PathBuf::from("debian/control"),
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(text: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(text))]
    }

    #[test]
    fn source() {
        let pass = vec![
            control("Source: example\nSection: devel\n\nPackage: example\nDescription: x\n"),
            control("Source: example\nSection:\n devel\n"),
            control("# comment:  spaced\nSource: example\n"),
            control("Source: example   \n"),
            control("Source: example\n Continuation:  line\n"),
        ];
        let fail = vec![
            control("Source:  example\n"),
            control("Source:example\n"),
            control("Section: devel\nPriority:   optional\n"),
            control("Source: example\nMaintainer:  Ex <ex@example.com>\n"),
            control("Source: example\nSection:\tdevel\n"),
            control("Source:\tvalue\n"),
        ];
        SourceTreeTester::new(DebianControlHasUnusualFieldSpacing, pass, fail).test_and_snapshot();
    }
}
