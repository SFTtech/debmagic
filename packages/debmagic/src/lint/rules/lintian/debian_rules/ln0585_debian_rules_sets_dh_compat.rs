use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::text::{lines_outside_targets, read_rules_from_tree, rules_location};

pub struct DebianRulesSetsDhCompat;

declare_rule! {
    /// `debian/rules` assigns `DH_COMPAT` outside of any rule.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) that come before any Policy target has been seen
    /// (`build`, `binary`, `clean` and the arch/indep variants, reached
    /// through a target line, `.PHONY`, or a `%` pattern). A line that starts
    /// (after an optional `export`) with `DH_COMPAT` and then `=` or `:=` is
    /// reported. Extra is empty. Location is `debian/rules` at the physical
    /// line where the logical line ends. Applies to the Source tree and Source
    /// package. A missing, unreadable, or non-UTF-8 `debian/rules` is not
    /// scanned.
    DebianRulesSetsDhCompat,
    code = "LN0585",
    tag = "debian-rules-sets-DH_COMPAT",
    default_selected = true,
    experimental = false,
}

static ASSIGNMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:export\s+)?DH_COMPAT\s*:?=").expect("valid regex"));

impl SourceRule for DebianRulesSetsDhCompat {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in lines_outside_targets(&contents) {
            if !ASSIGNMENT.is_match(&line) {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn("").with_location(rules_location(number)));
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
            rules("%:\n\tdh $@\n"),
            // Reading the variable is fine.
            rules("FOO = $(DH_COMPAT)\n%:\n\tdh $@\n"),
            // `+=` and `?=` are not matched.
            rules("DH_COMPAT += 9\n%:\n\tdh $@\n"),
            rules("DH_COMPAT ?= 9\n%:\n\tdh $@\n"),
            // A different variable.
            rules("DH_COMPAT_LEVEL = 9\n%:\n\tdh $@\n"),
            rules("MY_DH_COMPAT = 9\n%:\n\tdh $@\n"),
            // Once a Policy target has been seen, assignments are not reported.
            rules("%:\n\tdh $@\n\nDH_COMPAT = 9\n"),
            rules("build:\n\ttrue\nexport DH_COMPAT := 9\n"),
            rules(".PHONY: binary\nDH_COMPAT = 9\n"),
            // Recipe lines are not assignments at the start of a line.
            rules("%:\n\tDH_COMPAT=9 dh $@\n"),
            // A comment is not an assignment.
            rules("# DH_COMPAT = 9\n%:\n\tdh $@\n"),
        ];
        let fail = vec![
            rules("DH_COMPAT = 9\n%:\n\tdh $@\n"),
            rules("DH_COMPAT=9\n%:\n\tdh $@\n"),
            rules("DH_COMPAT := 9\n%:\n\tdh $@\n"),
            rules("export DH_COMPAT = 9\n%:\n\tdh $@\n"),
            rules("  export  DH_COMPAT\t=\t9\n%:\n\tdh $@\n"),
            rules("export DH_COMPAT=\n%:\n\tdh $@\n"),
            // Continuations join; the report points at the last physical line.
            rules("DH_COMPAT \\\n  = 9\n%:\n\tdh $@\n"),
            // One report per assignment.
            rules("DH_COMPAT = 7\nexport DH_COMPAT = 9\n%:\n\tdh $@\n"),
            // The report is made even when no target follows at all.
            rules("DH_COMPAT = 9\n"),
        ];
        SourceTreeTester::new(DebianRulesSetsDhCompat, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "%:\n\tdh $@\n\nDH_COMPAT = 9\n",
        ))];
        let fail = vec![SourcePackageCase::from(rules(
            "DH_COMPAT = 9\n%:\n\tdh $@\n",
        ))];
        SourcePackageTester::new(DebianRulesSetsDhCompat, pass, fail).test_and_snapshot();
    }
}
