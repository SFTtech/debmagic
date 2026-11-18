use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::text::{lines_outside_targets, read_rules_from_tree, rules_location};

pub struct DebianRulesSetsDpkgArchitectureVariable;

declare_rule! {
    /// `debian/rules` assigns a variable that `dpkg-architecture` provides.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) that come before any Policy target has been seen
    /// (`build`, `binary`, `clean` and the arch/indep variants, reached
    /// through a target line, `.PHONY`, a known helper makefile, or a `%`
    /// pattern). A line that starts (after an optional `export`) with
    /// `DEB_HOST_`, `DEB_BUILD_`, or `DEB_TARGET_` followed by `ARCH`,
    /// `MULTIARCH`, or `GNU` and then `=` or `:=` is reported. Extra is the
    /// variable name. Location is `debian/rules` at the physical line where
    /// the logical line ends. Applies to the Source tree and Source package. A
    /// missing, unreadable, or non-UTF-8 `debian/rules` is not scanned.
    DebianRulesSetsDpkgArchitectureVariable,
    code = "LN0581",
    tag = "debian-rules-sets-dpkg-architecture-variable",
    default_selected = true,
    experimental = false,
}

static ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:export\s+)?(DEB_(?:HOST|BUILD|TARGET)_(?:ARCH|MULTIARCH|GNU)[A-Z_]*)\s*:?=")
        .expect("valid regex")
});

impl SourceRule for DebianRulesSetsDpkgArchitectureVariable {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in lines_outside_targets(&contents) {
            let Some(found) = ASSIGNMENT.captures(&line) else {
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
            // Reading the variable is fine; only setting it is reported.
            rules("%:\n\tdh $@\n"),
            rules("export FOO = $(DEB_HOST_ARCH)\n%:\n\tdh $@\n"),
            // Variables that are not architecture facts.
            rules("DEB_HOST_OS = linux\n%:\n\tdh $@\n"),
            rules("FOO_DEB_HOST_ARCH = x\n%:\n\tdh $@\n"),
            // Once a Policy target has been seen, assignments are not reported.
            rules("%:\n\tdh $@\n\nDEB_HOST_ARCH = amd64\n"),
            rules("build:\n\ttrue\nDEB_BUILD_GNU_TYPE := x86_64-linux-gnu\n"),
            rules(".PHONY: clean\nDEB_HOST_ARCH = amd64\n"),
            // A comment is not an assignment.
            rules("# DEB_HOST_ARCH = amd64\n%:\n\tdh $@\n"),
            // Recipe lines are not assignments at the start of a line.
            rules("%:\n\tDEB_HOST_ARCH=amd64 dh $@\n"),
        ];
        let fail = vec![
            rules("DEB_HOST_ARCH = amd64\n%:\n\tdh $@\n"),
            rules("DEB_BUILD_ARCH := amd64\n%:\n\tdh $@\n"),
            rules("export DEB_HOST_MULTIARCH = x86_64-linux-gnu\n%:\n\tdh $@\n"),
            rules("DEB_TARGET_GNU_TYPE=x86_64-linux-gnu\n%:\n\tdh $@\n"),
            rules("  DEB_HOST_ARCH_BITS = 64\n%:\n\tdh $@\n"),
            rules("DEB_HOST_ARCH_CPU\t:=\tamd64\n%:\n\tdh $@\n"),
            // Several assignments, one report each.
            rules("DEB_HOST_ARCH = a\nDEB_BUILD_ARCH = b\n%:\n\tdh $@\n"),
            // Continuations join; the report points at the last physical line.
            rules("DEB_HOST_ARCH \\\n  = amd64\n%:\n\tdh $@\n"),
            // Conditional blocks do not count as targets.
            rules("ifeq (1,1)\nDEB_HOST_ARCH = amd64\nendif\n%:\n\tdh $@\n"),
            // The suffix after ARCH may be anything made of capitals.
            rules("DEB_HOST_ARCH_OS = linux\n%:\n\tdh $@\n"),
            // An included makefile that is not known provides no target here.
            rules("include /usr/share/cdbs/1/rules/debhelper.mk\nDEB_HOST_ARCH = amd64\n"),
            // The report is made even when no target follows at all.
            rules("DEB_HOST_ARCH = amd64\n"),
        ];
        SourceTreeTester::new(DebianRulesSetsDpkgArchitectureVariable, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "%:\n\tdh $@\n\nDEB_HOST_ARCH = amd64\n",
        ))];
        let fail = vec![SourcePackageCase::from(rules(
            "DEB_HOST_ARCH = amd64\n%:\n\tdh $@\n",
        ))];
        SourcePackageTester::new(DebianRulesSetsDpkgArchitectureVariable, pass, fail)
            .test_and_snapshot();
    }
}
