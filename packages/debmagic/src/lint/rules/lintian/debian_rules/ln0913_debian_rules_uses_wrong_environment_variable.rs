use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree, rules_location};

pub struct DebianRulesUsesWrongEnvironmentVariable;

declare_rule! {
    /// `debian/rules` reads or sets a user-only `DEB_*FLAGS_SET` or
    /// `DEB_*FLAGS_APPEND` variable.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for `DEB_` followed by one or more characters
    /// that are not `_`, a space, or a tab, then `FLAGS_`, then `SET` or
    /// `APPEND` at a word boundary. So `DEB_CFLAGS_SET` and
    /// `DEB_LDFLAGS_APPEND` match, and `DEB_CFLAGS_MAINT_SET` does not. One
    /// report per line. Extra is empty. Location is `debian/rules` at the
    /// physical line where the logical line ends. Applies to the Source tree
    /// and Source package. A missing, unreadable, or non-UTF-8 `debian/rules`
    /// is not scanned. A symlinked `debian/rules` is not followed for its
    /// contents (lintian follows it).
    DebianRulesUsesWrongEnvironmentVariable,
    code = "LN0913",
    tag = "debian-rules-uses-wrong-environment-variable",
    default_selected = true,
    experimental = false,
}

static USER_ONLY_FLAGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bDEB_[^_ \t]+FLAGS_(?:SET|APPEND)\b").expect("valid regex"));

impl SourceRule for DebianRulesUsesWrongEnvironmentVariable {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            if USER_ONLY_FLAGS.is_match(&line) {
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
            // The maintainer variants are the intended ones.
            rules("DEB_CFLAGS_MAINT_APPEND = -Wall\n"),
            rules("DEB_CFLAGS_MAINT_SET = -O2\n"),
            rules("DEB_BUILD_MAINT_OPTIONS = hardening=+all\n"),
            // `SETTING` is not `SET` at a word boundary.
            rules("DEB_CFLAGS_SETTING = x\n"),
            // A comment is not a use.
            rules("# DEB_CFLAGS_SET = -O2\n"),
        ];
        let fail = vec![
            rules("DEB_CFLAGS_SET = -O2\n"),
            rules("export DEB_CPPFLAGS_APPEND += -DX\n"),
            rules("DEB_LDFLAGS_APPEND:= -Wl,--as-needed\n"),
            rules("DEB_CXXFLAGS_SET = -O2\n"),
            // Continuations join; the report points at the last physical line.
            rules("DEB_CFLAGS_SET = \\\n  -O2\n"),
        ];
        SourceTreeTester::new(DebianRulesUsesWrongEnvironmentVariable, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "DEB_CFLAGS_MAINT_APPEND = -Wall\n",
        ))];
        let fail = vec![SourcePackageCase::from(rules("DEB_CFLAGS_SET = -O2\n"))];
        SourcePackageTester::new(DebianRulesUsesWrongEnvironmentVariable, pass, fail)
            .test_and_snapshot();
    }
}
