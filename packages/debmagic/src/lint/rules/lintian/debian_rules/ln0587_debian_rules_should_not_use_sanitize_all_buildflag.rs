use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree, rules_location};

pub struct DebianRulesShouldNotUseSanitizeAllBuildflag;

declare_rule! {
    /// `debian/rules` enables every sanitizer through `DEB_BUILD_MAINT_OPTIONS`.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for a line that starts (after an optional
    /// `export`) with `DEB_BUILD_MAINT_OPTIONS`, then `=` or `:=`, and later
    /// the word `sanitize=+all`. Extra is empty. Location is `debian/rules` at
    /// the physical line where the logical line ends. Applies to the Source
    /// tree and Source package. A missing, unreadable, or non-UTF-8
    /// `debian/rules` is not scanned.
    DebianRulesShouldNotUseSanitizeAllBuildflag,
    code = "LN0587",
    tag = "debian-rules-should-not-use-sanitize-all-buildflag",
    default_selected = true,
    experimental = false,
}

static SANITIZE_ALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:export\s+)?DEB_BUILD_MAINT_OPTIONS\s*:?=.*\bsanitize=\+all\b")
        .expect("valid regex")
});

impl SourceRule for DebianRulesShouldNotUseSanitizeAllBuildflag {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            if !SANITIZE_ALL.is_match(&line) {
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
            rules("export DEB_BUILD_MAINT_OPTIONS = hardening=+all\n"),
            rules("export DEB_BUILD_MAINT_OPTIONS = sanitize=+address\n"),
            rules("export DEB_BUILD_MAINT_OPTIONS = sanitize=-all\n"),
            // `+=` and `?=` are not matched.
            rules("DEB_BUILD_MAINT_OPTIONS += sanitize=+all\n"),
            rules("DEB_BUILD_MAINT_OPTIONS ?= sanitize=+all\n"),
            // The flag has to be a word of its own.
            rules("export DEB_BUILD_MAINT_OPTIONS = sanitize=+allx\n"),
            // The assignment has to start the line.
            rules("\techo DEB_BUILD_MAINT_OPTIONS = sanitize=+all\n"),
            rules("FOO = DEB_BUILD_MAINT_OPTIONS = sanitize=+all\n"),
            // A comment is not an assignment.
            rules("# export DEB_BUILD_MAINT_OPTIONS = sanitize=+all\n"),
        ];
        let fail = vec![
            rules("export DEB_BUILD_MAINT_OPTIONS = sanitize=+all\n"),
            rules("DEB_BUILD_MAINT_OPTIONS = sanitize=+all\n"),
            rules("DEB_BUILD_MAINT_OPTIONS := hardening=+all sanitize=+all\n"),
            rules("DEB_BUILD_MAINT_OPTIONS=sanitize=+all,hardening=+all\n"),
            rules("  export DEB_BUILD_MAINT_OPTIONS\t:=\tsanitize=+all\n"),
            // Continuations join; the report points at the last physical line.
            rules("export DEB_BUILD_MAINT_OPTIONS = hardening=+all \\\n  sanitize=+all\n"),
            // One report per line.
            rules(
                "DEB_BUILD_MAINT_OPTIONS = sanitize=+all\nexport DEB_BUILD_MAINT_OPTIONS = sanitize=+all\n",
            ),
        ];
        SourceTreeTester::new(DebianRulesShouldNotUseSanitizeAllBuildflag, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "export DEB_BUILD_MAINT_OPTIONS = hardening=+all\n",
        ))];
        let fail = vec![SourcePackageCase::from(rules(
            "export DEB_BUILD_MAINT_OPTIONS = sanitize=+all\n",
        ))];
        SourcePackageTester::new(DebianRulesShouldNotUseSanitizeAllBuildflag, pass, fail)
            .test_and_snapshot();
    }
}
