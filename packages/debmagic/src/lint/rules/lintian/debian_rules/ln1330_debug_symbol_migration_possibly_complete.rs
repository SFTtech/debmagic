use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::text::{complete_lines, read_rules_from_tree};

pub struct DebugSymbolMigrationPossiblyComplete;

declare_rule! {
    /// A `debian/rules` `dh_strip` call passes a `--dbgsym-migration` or
    /// `--ddeb-migration` argument.
    ///
    /// Such arguments migrate packages to automatic debug symbols, which
    /// first became available in December 2015. If the call was already in
    /// the stable release, it can possibly be removed. The extra is the
    /// matched option with its value, quotes included (`--dbgsym-migration='a
    /// << 1'`). Comment lines are ignored and `\` continuations are joined;
    /// the location is the physical line where the logical line ends. Applies
    /// to the Source tree and Source package.
    DebugSymbolMigrationPossiblyComplete,
    code = "LN1330",
    tag = "debug-symbol-migration-possibly-complete",
    default_selected = false,
    experimental = false,
}

/// `dh_strip` followed by a migration option; group 1 is the option.
static MIGRATION_OPTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"dh_strip\b.*(--(?:ddeb|dbgsym)-migration=(?:'[^']*'|\S*))").expect("valid regex")
});

impl SourceRule for DebugSymbolMigrationPossiblyComplete {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            let Some(captures) = MIGRATION_OPTION.captures(&line) else {
                continue;
            };
            ctx.diagnostic(
                Diagnostic::warn(&captures[1])
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path: PathBuf::from("debian/rules"),
                        line: Some(number),
                        column: None,
                    }),
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
                TestFile::new(format!(
                    "#!/usr/bin/make -f\n\n%:\n\tdh $@\n\noverride_dh_strip:\n{body}"
                )),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            vec![("debian/rules", TestFile::new(""))],
            rules("\tdh_strip\n"),
            rules("\tdh_strip --no-automatic-dbgsym\n"),
            // Not on a `dh_strip` line.
            rules("\tdh_shlibdeps --dbgsym-migration=foo\n"),
            // A comment is not a recipe.
            rules("#\tdh_strip --dbgsym-migration=foo\n"),
            // An option that is not a migration option.
            rules("\tdh_strip --dbgsym-migration-x\n"),
        ];
        let fail = vec![
            rules("\tdh_strip --ddeb-migration=foo\n"),
            rules("\tdh_strip --dbgsym-migration=foo\n"),
            rules("\tdh_strip --dbgsym-migration='apostrophe'\n"),
            rules("\tdh_strip --dbgsym-migration='apostrophe << 1'\n"),
            // Continuations join into one line.
            rules("\tdh_strip \\\n\t  --ddeb-migration=foo\n"),
        ];
        SourceTreeTester::new(DebugSymbolMigrationPossiblyComplete, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("\tdh_strip\n"))];
        let fail = vec![SourcePackageCase::from(rules(
            "\tdh_strip --dbgsym-migration=foo\n",
        ))];
        SourcePackageTester::new(DebugSymbolMigrationPossiblyComplete, pass, fail)
            .test_and_snapshot();
    }
}
