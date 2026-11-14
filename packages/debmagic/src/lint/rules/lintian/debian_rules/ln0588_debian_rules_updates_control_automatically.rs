use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree, rules_location};

pub struct DebianRulesUpdatesControlAutomatically;

declare_rule! {
    /// `debian/rules` asks CDBS to rewrite `debian/control` during the build.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for a line that starts with
    /// `DEB_AUTO_UPDATE_DEBIAN_CONTROL`, then `=`, then `yes` (a longer word
    /// starting with `yes` also matches). Extra is empty. Location is
    /// `debian/rules` at the physical line where the logical line ends.
    /// Applies to the Source tree and Source package. A missing, unreadable,
    /// or non-UTF-8 `debian/rules` is not scanned.
    DebianRulesUpdatesControlAutomatically,
    code = "LN0588",
    tag = "debian-rules-updates-control-automatically",
    default_selected = true,
    experimental = false,
}

static AUTO_UPDATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*DEB_AUTO_UPDATE_DEBIAN_CONTROL\s*=\s*yes").expect("valid regex")
});

impl SourceRule for DebianRulesUpdatesControlAutomatically {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            if !AUTO_UPDATE.is_match(&line) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error("").with_location(rules_location(number)));
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
                TestFile::new(format!("#!/usr/bin/make -f\n\n%:\n\tdh $@\n\n{body}")),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            vec![("debian/rules", TestFile::new(""))],
            rules(""),
            rules("DEB_AUTO_UPDATE_DEBIAN_CONTROL = no\n"),
            // Only a plain `=` assignment at the start of the line.
            rules("export DEB_AUTO_UPDATE_DEBIAN_CONTROL = yes\n"),
            rules("DEB_AUTO_UPDATE_DEBIAN_CONTROL := yes\n"),
            rules("DEB_AUTO_UPDATE_DEBIAN_CONTROL ?= yes\n"),
            rules("FOO_DEB_AUTO_UPDATE_DEBIAN_CONTROL = yes\n"),
            rules("DEB_AUTO_UPDATE_DEBIAN_CONTROL_X = yes\n"),
            // A comment is not an assignment.
            rules("# DEB_AUTO_UPDATE_DEBIAN_CONTROL = yes\n"),
        ];
        let fail = vec![
            rules("DEB_AUTO_UPDATE_DEBIAN_CONTROL = yes\n"),
            rules("DEB_AUTO_UPDATE_DEBIAN_CONTROL=yes\n"),
            rules("  DEB_AUTO_UPDATE_DEBIAN_CONTROL\t=\tyes\n"),
            // Anything after `yes` is ignored.
            rules("DEB_AUTO_UPDATE_DEBIAN_CONTROL = yesterday\n"),
            rules("DEB_AUTO_UPDATE_DEBIAN_CONTROL = yes # keep\n"),
            // Continuations join; the report points at the last physical line.
            rules("DEB_AUTO_UPDATE_DEBIAN_CONTROL = \\\n  yes\n"),
            // One report per assignment.
            rules("DEB_AUTO_UPDATE_DEBIAN_CONTROL = yes\nDEB_AUTO_UPDATE_DEBIAN_CONTROL = yes\n"),
        ];
        SourceTreeTester::new(DebianRulesUpdatesControlAutomatically, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "DEB_AUTO_UPDATE_DEBIAN_CONTROL = no\n",
        ))];
        let fail = vec![SourcePackageCase::from(rules(
            "DEB_AUTO_UPDATE_DEBIAN_CONTROL = yes\n",
        ))];
        SourcePackageTester::new(DebianRulesUpdatesControlAutomatically, pass, fail)
            .test_and_snapshot();
    }
}
