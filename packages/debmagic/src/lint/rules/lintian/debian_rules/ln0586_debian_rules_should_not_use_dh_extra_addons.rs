use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree, rules_location};

pub struct DebianRulesShouldNotUseDhExtraAddons;

declare_rule! {
    /// `debian/rules` assigns `DH_EXTRA_ADDONS`.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for a line that starts with `DH_EXTRA_ADDONS`
    /// and then `=`; `export` in front, `:=`, `+=`, and `?=` do not match.
    /// Extra is everything after the `=` (leading whitespace after it is
    /// consumed by the match, trailing text is kept). Location is
    /// `debian/rules` at the physical line where the logical line ends.
    /// Applies to the Source tree and Source package. A missing, unreadable,
    /// or non-UTF-8 `debian/rules` is not scanned.
    DebianRulesShouldNotUseDhExtraAddons,
    code = "LN0586",
    tag = "debian-rules-should-not-use-DH_EXTRA_ADDONS",
    default_selected = true,
    experimental = false,
}

static ADDONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*DH_EXTRA_ADDONS\s*=\s*(.*)$").expect("valid regex"));

impl SourceRule for DebianRulesShouldNotUseDhExtraAddons {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            let Some(found) = ADDONS.captures(&line) else {
                continue;
            };
            ctx.diagnostic(
                Diagnostic::warn(found[1].to_string()).with_location(rules_location(number)),
            );
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
            // Only the plain `=` form at the start of a line matches.
            rules("export DH_EXTRA_ADDONS = systemd\n"),
            rules("DH_EXTRA_ADDONS := systemd\n"),
            rules("DH_EXTRA_ADDONS += systemd\n"),
            rules("DH_EXTRA_ADDONS ?= systemd\n"),
            // Reading the variable, a different variable, or a recipe line.
            rules("FOO = $(DH_EXTRA_ADDONS)\n"),
            rules("DH_EXTRA_ADDONS_MORE = systemd\n"),
            rules("MY_DH_EXTRA_ADDONS = systemd\n"),
            // A comment is not an assignment.
            rules("# DH_EXTRA_ADDONS = systemd\n"),
        ];
        let fail = vec![
            rules("DH_EXTRA_ADDONS = systemd\n"),
            rules("DH_EXTRA_ADDONS=systemd\n"),
            rules("  DH_EXTRA_ADDONS\t=\tautoreconf systemd\n"),
            // The extra keeps the rest of the line, including trailing blanks
            // and comments.
            rules("DH_EXTRA_ADDONS = systemd  # why\n"),
            rules("DH_EXTRA_ADDONS =\n"),
            // The report is made inside a recipe body or after the targets too:
            // the line only has to start with the variable.
            rules("override_dh_auto_build:\nDH_EXTRA_ADDONS = systemd\n"),
            // Continuations join; the report points at the last physical line.
            rules("DH_EXTRA_ADDONS = systemd \\\n  autoreconf\n"),
            // Leading whitespace includes a recipe tab, so a recipe line counts.
            rules("\tDH_EXTRA_ADDONS=systemd dh $@\n"),
            // One report per assignment.
            rules("DH_EXTRA_ADDONS = a\nDH_EXTRA_ADDONS = b\n"),
        ];
        SourceTreeTester::new(DebianRulesShouldNotUseDhExtraAddons, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("FOO = bar\n"))];
        let fail = vec![SourcePackageCase::from(rules(
            "DH_EXTRA_ADDONS = systemd\n",
        ))];
        SourcePackageTester::new(DebianRulesShouldNotUseDhExtraAddons, pass, fail)
            .test_and_snapshot();
    }
}
