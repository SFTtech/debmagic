use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree, rules_location};

pub struct DebianRulesUsesSpecialShellVariable;

declare_rule! {
    /// `debian/rules` uses the make variable `$(_)` or `${_}`.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for `$(_)` or `${_}`. The two brackets are
    /// independent, as in lintian, so `$(_}` also matches. `$_` without
    /// brackets and `$(__)` do not. One report per line. Extra is empty.
    /// Location is `debian/rules` at the physical line where the logical line
    /// ends. Applies to the Source tree and Source package. A missing,
    /// unreadable, or non-UTF-8 `debian/rules` is not scanned. A symlinked
    /// `debian/rules` is not followed for its contents (lintian follows it).
    DebianRulesUsesSpecialShellVariable,
    code = "LN0912",
    tag = "debian-rules-uses-special-shell-variable",
    default_selected = true,
    experimental = false,
}

static SPECIAL_VARIABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$[({]_[)}]").expect("valid regex"));

impl SourceRule for DebianRulesUsesSpecialShellVariable {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            if SPECIAL_VARIABLE.is_match(&line) {
                ctx.diagnostic(Diagnostic::warn("").with_location(rules_location(number)));
            }
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
            rules("\t@echo $(DEB_HOST_ARCH)\n"),
            // Without brackets `$_` is an ordinary shell variable.
            rules("\t@echo $_\n"),
            // Longer names and `$(__)` are not the special variable.
            rules("\t@echo ${_x} $(__)\n"),
            // A comment is not a use.
            rules("# $(_)\n"),
        ];
        let fail = vec![
            rules("\t@echo $(_)\n"),
            rules("\t@echo ${_}\n"),
            // The brackets are independent.
            rules("\t@echo $(_}\n"),
            // Two uses on one line are one report.
            rules("\t@echo $(_) ${_}\n"),
            // Continuations join; the report points at the last physical line.
            rules("\t@echo \\\n\t$(_)\n"),
        ];
        SourceTreeTester::new(DebianRulesUsesSpecialShellVariable, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("\t@echo ok\n"))];
        let fail = vec![SourcePackageCase::from(rules("\t@echo $(_)\n"))];
        SourcePackageTester::new(DebianRulesUsesSpecialShellVariable, pass, fail)
            .test_and_snapshot();
    }
}
