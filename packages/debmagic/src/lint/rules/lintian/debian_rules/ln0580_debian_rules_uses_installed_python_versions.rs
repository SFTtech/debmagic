use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree, rules_location};

pub struct DebianRulesUsesInstalledPythonVersions;

declare_rule! {
    /// `debian/rules` asks `py3versions` for the installed interpreters.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for `py3versions` followed by `--installed` or a
    /// short option cluster that contains `i` (`-i`, `-vi`, `-iv`). Extra is
    /// that `py3versions` invocation. Location is `debian/rules` at the
    /// physical line where the logical line ends. One report per logical line.
    /// Applies to the Source tree and Source package. A missing, unreadable,
    /// or non-UTF-8 `debian/rules` is not scanned.
    DebianRulesUsesInstalledPythonVersions,
    code = "LN0580",
    tag = "debian-rules-uses-installed-python-versions",
    default_selected = true,
    experimental = false,
}

static INSTALLED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(py3versions\s+(?:[\w\s-]*--installed|-\w*i\w*))").expect("valid regex")
});

impl SourceRule for DebianRulesUsesInstalledPythonVersions {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            let Some(found) = INSTALLED.captures(&line) else {
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
            rules("\tpy3versions -r debian/control\n"),
            rules("\tpy3versions --requested\n"),
            // The supported interpreters are a different Tag.
            rules("\tpy3versions -s\n"),
            rules("\tpy3versions --supported\n"),
            // `py3versions` needs whitespace and then the option.
            rules("\tpy3versions\n"),
            rules("\tpy3versions --version\n"),
            // A comment is not a call.
            rules("# py3versions -i\n"),
            rules("\t# py3versions -i\n"),
            // Another command does not count.
            rules("\tpy2versions -i\n"),
        ];
        let fail = vec![
            rules("\tpy3versions -i\n"),
            rules("\tpy3versions -vi\n"),
            rules("\tpy3versions -iv\n"),
            rules("\tpy3versions --installed\n"),
            rules("\tpy3versions --verbose --installed\n"),
            rules("PYVERS = $(shell py3versions -vi)\n"),
            // The cluster may carry other letters around the `i`.
            rules("\tpy3versions -avib\n"),
            // Continuations join; the report points at the last physical line.
            rules("\tpy3versions \\\n\t  --installed\n"),
            // One report per logical line.
            rules("\tpy3versions -i\n\tpy3versions --installed\n"),
            // A comment does not hide a later call.
            rules("# py3versions -i\n\tpy3versions -i\n"),
        ];
        SourceTreeTester::new(DebianRulesUsesInstalledPythonVersions, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("\tpy3versions -r\n"))];
        let fail = vec![SourcePackageCase::from(rules("\tpy3versions -vi\n"))];
        SourcePackageTester::new(DebianRulesUsesInstalledPythonVersions, pass, fail)
            .test_and_snapshot();
    }
}
