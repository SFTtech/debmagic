use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::control::{field, paragraphs, read};

pub struct NoOpTestsuite;

declare_rule! {
    /// The only autopkgtest command is `true` or `/bin/true`.
    ///
    /// `debian/tests/control` must be one Deb822 paragraph whose
    /// `Test-Command`, after unfolding, is optional whitespace, an optional
    /// `/bin/`, then `true`. Extra is empty. The Location is
    /// `debian/tests/control`. Applies to the Source tree and Source package.
    NoOpTestsuite,
    code = "LN0399",
    tag = "no-op-testsuite",
    default_selected = true,
    experimental = false,
}

static NO_OP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:/bin/)?true\s*$").expect("regex"));

/// Line of `name:` in the tests control file. The hint's only text is that
/// pointer, so the line is part of the Parity extra.
fn field_line(text: &str, name: &str) -> Option<u32> {
    let prefix = format!("{}:", name.to_ascii_lowercase());
    text.lines().enumerate().find_map(|(index, line)| {
        line.to_ascii_lowercase()
            .starts_with(&prefix)
            .then_some(u32::try_from(index + 1).unwrap_or(u32::MAX))
    })
}

impl SourceRule for NoOpTestsuite {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(text) = read(files, "debian/tests/control") else {
            return;
        };
        let Some(sections) = paragraphs(&text) else {
            return;
        };
        if sections.len() != 1 {
            return;
        }
        let Some(command) = field(&sections[0], "Test-Command") else {
            return;
        };
        if !NO_OP.is_match(command.trim()) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
            path: PathBuf::from("debian/tests/control"),
            line: field_line(&text, "Test-Command"),
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/tests/control", TestFile::new(body)),
        ]
    }

    const CONTROL: &str = "\
Source: example
Maintainer: Example <ex@example.com>

Package: example
Architecture: all
Description: example
 extra
";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no tests"))],
            tree("Test-Command: /usr/bin/true\nDepends: @\n"),
            tree("Tests: smoke\nDepends: @\n"),
            tree("Test-Command: true && true\n"),
            tree("Test-Command: /bin/true\n\nTest-Command: /bin/false\n"),
        ];
        let fail = vec![
            tree("Test-Command: /bin/true\nDepends: @\n"),
            tree("Test-Command: true\n"),
            tree("Test-Command:   /bin/true   \n"),
        ];
        SourceTreeTester::new(NoOpTestsuite, pass, fail).test_and_snapshot();
    }
}
