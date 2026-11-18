use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree, rules_location};

pub struct DebianRulesUsesDebBuildOpts;

declare_rule! {
    /// `debian/rules` expands `DEB_BUILD_OPTS`, which dpkg does not set.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for `$(DEB_BUILD_OPTS)` or `${DEB_BUILD_OPTS}`
    /// (the brackets are matched independently). Extra is empty. Location is
    /// `debian/rules` at the physical line where the logical line ends. One
    /// report per logical line. Applies to the Source tree and Source
    /// package. A missing, unreadable, or non-UTF-8 `debian/rules` is not
    /// scanned.
    DebianRulesUsesDebBuildOpts,
    code = "LN0589",
    tag = "debian-rules-uses-deb-build-opts",
    default_selected = true,
    experimental = false,
}

static DEB_BUILD_OPTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$[({]DEB_BUILD_OPTS[)}]").expect("valid regex"));

impl SourceRule for DebianRulesUsesDebBuildOpts {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            if !DEB_BUILD_OPTS.is_match(&line) {
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
            rules("\techo $(DEB_BUILD_OPTIONS)\n"),
            rules("\techo ${DEB_BUILD_OPTIONS}\n"),
            // Bare `$DEB_BUILD_OPTS` is not the make form lintian matches.
            rules("\techo $DEB_BUILD_OPTS\n"),
            rules("\techo $(DEB_BUILD_OPTS_EXTRA)\n"),
            rules("\techo $( DEB_BUILD_OPTS )\n"),
            rules("DEB_BUILD_OPTS = x\n"),
            // A comment is not a use.
            rules("# $(DEB_BUILD_OPTS)\n"),
        ];
        let fail = vec![
            rules("\techo $(DEB_BUILD_OPTS)\n"),
            rules("\techo ${DEB_BUILD_OPTS}\n"),
            // Mismatched brackets still match.
            rules("\techo $(DEB_BUILD_OPTS}\n"),
            rules("FOO = $(findstring nocheck,$(DEB_BUILD_OPTS))\n"),
            // Continuations join; the report points at the last physical line.
            rules("\techo $(DEB_BUILD_\\\nOPTS)\n"),
            // One report per logical line.
            rules("\techo $(DEB_BUILD_OPTS) ${DEB_BUILD_OPTS}\n"),
        ];
        SourceTreeTester::new(DebianRulesUsesDebBuildOpts, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "\techo $(DEB_BUILD_OPTIONS)\n",
        ))];
        let fail = vec![SourcePackageCase::from(rules("\techo $(DEB_BUILD_OPTS)\n"))];
        SourcePackageTester::new(DebianRulesUsesDebBuildOpts, pass, fail).test_and_snapshot();
    }
}
