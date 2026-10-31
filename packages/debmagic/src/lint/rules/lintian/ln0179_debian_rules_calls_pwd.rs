use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::debian_rules::{complete_lines, read_rules_from_tree};

pub struct DebianRulesCallsPwd;

declare_rule! {
    /// `debian/rules` refers to the current directory as `$(PWD)` or `${PWD}`.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined). `$(PWD)` and `${PWD}` match, including a
    /// mismatched closer. `$PWD` without brackets does not. One report per
    /// line. Extra is empty. Location is `debian/rules` at the physical line
    /// where the logical line ends. Applies to the Source tree and Source
    /// package. A missing, unreadable, or non-UTF-8 `debian/rules` is not
    /// scanned.
    DebianRulesCallsPwd,
    code = "LN0179",
    tag = "debian-rules-calls-pwd",
    default_selected = true,
    experimental = false,
}

/// `$(PWD)` or `${PWD}`. The brackets are independent, as in lintian.
static PWD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$[({]PWD[)}]").expect("valid regex"));

impl SourceRule for DebianRulesCallsPwd {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            if !PWD.is_match(&line) {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/rules"),
                line: Some(number),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn rules(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(CONTROL)),
            (
                "debian/rules",
                TestFile::new(format!("#!/usr/bin/make -f\n\n{body}")),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            vec![("debian/rules", TestFile::new(""))],
            rules(""),
            rules("\techo $(CURDIR)\n"),
            rules("\techo ${CURDIR}\n"),
            // Bare `$PWD` is not the make variable form lintian matches.
            rules("\techo $PWD\n"),
            rules("\techo $(PWD_DIR)\n"),
            rules("\techo ${PWD_DIR}\n"),
            rules("\techo $( PWD )\n"),
            // Comments are dropped before the scan.
            rules("# $(PWD)\n"),
            rules("  # ${PWD}\n"),
            rules("\t# $(PWD)\n"),
            // A comment between the continued lines keeps the next line's indent.
            rules("\techo $(PW\\\n# comment\n\tD)\n"),
        ];
        let fail = vec![
            rules("\techo $(PWD)\n"),
            rules("\techo ${PWD}\n"),
            // Mismatched brackets still match; the two classes are independent.
            rules("\techo $(PWD}\n"),
            rules("FOO = $(PWD)\n"),
            rules("\techo foo$(PWD)bar ${PWD}\n"),
            // Continuations join; the report points at the last physical line.
            rules("\techo $(PW\\\nD)\n"),
            // One report per line.
            rules("\techo $(PWD)\n\techo ${PWD}\n"),
            // A comment does not hide a later call, and does not break a join
            // when the following physical line has no indent.
            rules("# $(PWD)\n\techo $(PWD)\n"),
            rules("\techo $(PW\\\n# still joining\nD)\n"),
        ];
        SourceTreeTester::new(DebianRulesCallsPwd, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("\techo $(CURDIR)\n"))];
        let fail = vec![SourcePackageCase::from(rules("\techo $(PWD)\n"))];
        SourcePackageTester::new(DebianRulesCallsPwd, pass, fail).test_and_snapshot();
    }
}
