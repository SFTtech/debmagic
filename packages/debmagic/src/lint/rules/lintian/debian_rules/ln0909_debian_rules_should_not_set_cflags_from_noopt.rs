use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::text::read_rules_from_tree;

pub struct DebianRulesShouldNotSetCFLAGSFromNoopt;

declare_rule! {
    /// `debian/rules` sets `CFLAGS` to `-O0` or `-O2` under a `noopt` test.
    ///
    /// Looks for one block in the whole file, comments included: an `ifeq` or
    /// `ifneq` on `$(findstring noopt,$(DEB_BUILD_OPTIONS))`, a tab-indented
    /// `CFLAGS += -O0` or `-O2`, `else`, a tab-indented `CFLAGS += -O0` or
    /// `-O2`, and `endif`, each on the next line (blank lines between them
    /// are allowed). Only that exact shape is reported, as lintian's pattern
    /// has it. Extra is empty. Location is `debian/rules` with no line.
    /// Applies to the Source tree and Source package. A missing, unreadable,
    /// or non-UTF-8 `debian/rules` is not scanned. A symlinked `debian/rules`
    /// is not followed for its contents (lintian follows it).
    DebianRulesShouldNotSetCFLAGSFromNoopt,
    code = "LN0909",
    tag = "debian-rules-should-not-set-CFLAGS-from-noopt",
    default_selected = true,
    experimental = false,
}

/// Lintian's `ifn?eq ... noopt ... else ... endif` block, read with its `/xsm`
/// flags. The line breaks are `\n` runs, as in the Perl source.
static NOOPT_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?xms)
        ^ ifn?eq \s+ [(] , \$ [(] findstring \s+ noopt , \$ [(] DEB_BUILD_OPTIONS [)] [)] [)] \n+
        \t+ CFLAGS \s+ \+ = \s+ -O[02] \n+
        else \n+
        \t+ CFLAGS \s+ \+ = \s+ -O[02] \n+
        endif $",
    )
    .expect("valid regex")
});

impl SourceRule for DebianRulesShouldNotSetCFLAGSFromNoopt {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        if NOOPT_BLOCK.is_match(&contents) {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/rules"),
                line: None,
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

    /// The `noopt` block, with `ifeq`, `-O0` and `-O2` as the Perl pattern wants.
    const NOOPT: &str = "ifeq (,$(findstring noopt,$(DEB_BUILD_OPTIONS)))\n\
         \tCFLAGS += -O0\n\
         else\n\
         \tCFLAGS += -O2\n\
         endif\n";

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
            // The optimisation level must be `-O0` or `-O2` in both branches.
            rules(
                "ifeq (,$(findstring noopt,$(DEB_BUILD_OPTIONS)))\n\tCFLAGS += -O1\nelse\n\tCFLAGS += -O2\nendif\n",
            ),
            // The condition must name `noopt`.
            rules(
                "ifeq (,$(findstring opt,$(DEB_BUILD_OPTIONS)))\n\tCFLAGS += -O0\nelse\n\tCFLAGS += -O2\nendif\n",
            ),
            // No `else` branch.
            rules("ifeq (,$(findstring noopt,$(DEB_BUILD_OPTIONS)))\n\tCFLAGS += -O0\nendif\n"),
            // A comment between the lines breaks the adjacency the pattern needs.
            rules(
                "ifeq (,$(findstring noopt,$(DEB_BUILD_OPTIONS)))\n\tCFLAGS += -O0\n# c\nelse\n\tCFLAGS += -O2\nendif\n",
            ),
            // Not tab-indented.
            rules(
                "ifeq (,$(findstring noopt,$(DEB_BUILD_OPTIONS)))\n  CFLAGS += -O0\nelse\n  CFLAGS += -O2\nendif\n",
            ),
            // A comment is not the block.
            rules("# ifeq (,$(findstring noopt,$(DEB_BUILD_OPTIONS)))\n"),
        ];
        let fail = vec![
            rules(NOOPT),
            // `ifneq` is the same test with the branches swapped.
            rules(
                "ifneq (,$(findstring noopt,$(DEB_BUILD_OPTIONS)))\n\tCFLAGS += -O0\nelse\n\tCFLAGS += -O2\nendif\n",
            ),
            // Whitespace around `+=` may vary, and `-O2` may come first.
            rules(
                "ifeq (,$(findstring noopt,$(DEB_BUILD_OPTIONS)))\n\tCFLAGS  +=  -O2\nelse\n\tCFLAGS +=  -O0\nendif\n",
            ),
            // Blank lines between the parts are allowed.
            rules(
                "ifeq (,$(findstring noopt,$(DEB_BUILD_OPTIONS)))\n\n\tCFLAGS += -O0\nelse\n\tCFLAGS += -O2\n\nendif\n",
            ),
        ];
        SourceTreeTester::new(DebianRulesShouldNotSetCFLAGSFromNoopt, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("CFLAGS += -O2\n"))];
        let fail = vec![SourcePackageCase::from(rules(NOOPT))];
        SourcePackageTester::new(DebianRulesShouldNotSetCFLAGSFromNoopt, pass, fail)
            .test_and_snapshot();
    }
}
