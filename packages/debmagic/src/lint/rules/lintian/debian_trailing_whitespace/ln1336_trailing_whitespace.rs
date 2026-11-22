use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct TrailingWhitespace;

declare_rule! {
    /// A packaging file has lines with trailing whitespace.
    ///
    /// Checked files: `debian/changelog` and `debian/control` (a trailing
    /// whitespace character of any kind), and `debian/rules` (a trailing
    /// space; trailing tabs are allowed for make). Only files whose bytes are
    /// valid UTF-8 are checked. The text is split on `\n` keeping empty
    /// fields. Each line ending in such whitespace is reported. Lines that
    /// are empty and trail the file are reported too, except the final empty
    /// field that a closing newline leaves behind. Extra is empty; the
    /// Location is the file and the line. A CRLF line ends in `\r`, which
    /// counts as whitespace. The `no-newline-at-end` case of lintian's Check is
    /// a different Tag and is not reported here. Applies to the Source tree and
    /// Source package.
    TrailingWhitespace,
    code = "LN1336",
    tag = "trailing-whitespace",
    default_selected = false,
    experimental = false,
}

/// The packaging files this Rule checks, with whether a trailing tab is
/// allowed (only `debian/rules` allows it).
const CHECKED: [(&str, bool); 3] = [
    ("debian/changelog", false),
    ("debian/control", false),
    ("debian/rules", true),
];

impl SourceRule for TrailingWhitespace {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let mut found = Vec::new();
        for (name, allows_tab) in CHECKED {
            let Some(entry) = files.get(name) else {
                continue;
            };
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            let Ok(text) = std::str::from_utf8(&bytes) else {
                continue;
            };
            for line in trailing_whitespace_lines(text, allows_tab) {
                found.push((PathBuf::from(name), line));
            }
        }
        for (path, line) in found {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

/// 1-based numbers of the lines that have trailing whitespace, then the
/// trailing empty lines, as lintian's Check collects them.
fn trailing_whitespace_lines(text: &str, allows_tab: bool) -> Vec<u32> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut reported = Vec::new();
    let mut empty_at_end: Vec<u32> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let trailing = if allows_tab {
            line.ends_with(' ')
        } else {
            line.ends_with(char::is_whitespace)
        };
        if trailing {
            reported.push(number);
        }
        if line.is_empty() {
            empty_at_end.push(number);
        } else {
            empty_at_end.clear();
        }
    }
    // The final empty field left by a closing newline is not reported.
    if lines.last().is_some_and(|line| line.is_empty()) {
        empty_at_end.pop();
    }
    reported.extend(empty_at_end);
    reported
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn files(changelog: &str, control: &str, rules: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/changelog", TestFile::new(changelog)),
            ("debian/control", TestFile::new(control)),
            ("debian/rules", TestFile::new(rules)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("foo \n"))],
            files("a\n", "Source: a\n", "all:\n"),
            // Trailing tabs are allowed in debian/rules.
            files("a\n", "Source: a\n", "all:\n\t\n"),
            // A file that is not UTF-8 is not checked.
            vec![(
                "debian/control",
                TestFile::from_bytes(b"Source: a \xff\n".to_vec()),
            )],
            // A trailing newline is not an empty line to report.
            files("a\n", "Source: a\n", "all:\n"),
            // An empty file.
            files("", "", ""),
        ];
        let fail = vec![
            files("a \n", "Source: a\n", "all:\n"),
            files("a\n", "Source: a\t\n", "all:\n"),
            files("a\n", "Source: a\n", "all: \n"),
            // CRLF line endings.
            files("a\r\nb\n", "Source: a\r\n", "all:\n"),
            // Trailing empty lines beyond the final newline are reported.
            files("a\n\n\n", "Source: a\n", "all:\n"),
            // A whitespace-only line is a trailing-whitespace line.
            files("a\n   \n", "Source: a\n", "all:\n"),
        ];
        SourceTreeTester::new(TrailingWhitespace, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(files(
            "a\n",
            "Source: a\n",
            "all:\n",
        ))];
        let fail = vec![SourcePackageCase::from(files(
            "a \n",
            "Source: a\n",
            "all:\n",
        ))];
        SourcePackageTester::new(TrailingWhitespace, pass, fail).test_and_snapshot();
    }
}
