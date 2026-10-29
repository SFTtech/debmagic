use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::debian_rules::{complete_lines, read_rules_from_tree};

pub struct DebianRulesIgnoresMakeCleanError;

declare_rule! {
    /// A `debian/rules` recipe calls `make clean` in a way that ignores errors.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for a recipe line (starting with a tab) that
    /// either starts with a `-` before `make` / `$(MAKE)` and later has
    /// `clean` (or `distclean`), or passes make a flag cluster containing `i`
    /// (`-i`, `-ki`) and later has `clean`. A cluster that starts with `C`
    /// (`-Ci`, a directory option) is not reported. Extra is empty. Location
    /// is `debian/rules` at the physical line where the logical line ends.
    /// Applies to the Source tree and Source package.
    DebianRulesIgnoresMakeCleanError,
    code = "LN0017",
    tag = "debian-rules-ignores-make-clean-error",
    default_selected = true,
    experimental = false,
}

/// `-` before make: errors of the whole line are ignored. No capture group.
static DASH_MAKE_CLEAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^\t\s*-(?:\$[({]MAKE[})]|make)\s.*(?:dist)?clean").expect("valid regex")
});
/// `-i` among make's flags; group 1 is the flag letters before the `i`.
static IGNORE_FLAG_MAKE_CLEAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^\t\s*(?:\$[({]MAKE[})]|make)\s(?:.*\s)?-(\w*)i.*(?:dist)?clean")
        .expect("valid regex")
});

impl SourceRule for DebianRulesIgnoresMakeCleanError {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            if !ignores_errors(&line) {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/rules"),
                line: Some(number),
                column: None,
            }));
        }
    }
}

/// lintian reuses `$1` after the `||`: the first form leaves it undefined.
fn ignores_errors(line: &str) -> bool {
    if DASH_MAKE_CLEAN.is_match(line) {
        return true;
    }
    let Some(captures) = IGNORE_FLAG_MAKE_CLEAN.captures(line) else {
        return false;
    };
    // `-C<dir>` followed by `i` is not an ignore-errors flag (#671537).
    !captures[1].starts_with('C')
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
                    "#!/usr/bin/make -f\n\n%:\n\tdh $@\n\nclean:\n{body}"
                )),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            vec![("debian/rules", TestFile::new(""))],
            rules("\t$(MAKE) clean\n"),
            rules("\t$(MAKE) distclean\n"),
            rules("\tmake clean\n"),
            rules("\t${MAKE} -C src clean\n"),
            // A directory option followed by an `i` in the same cluster.
            rules("\t$(MAKE) -Ci clean\n"),
            rules("\t$(MAKE) -C dir clean\n"),
            // No `clean`.
            rules("\t-$(MAKE) install\n"),
            rules("\tmake -i install\n"),
            // Not a recipe line, or not make.
            rules("clean: -make clean\n"),
            rules("    -make clean\n"),
            rules("\tdh_clean -i\n"),
            rules("\t-dh_auto_clean\n"),
            // A comment is not a recipe.
            rules("#\t-make clean\n"),
            // The `-` must come right before `make`.
            rules("\t- make clean\n"),
            // The first `-…i` cluster that fits decides: `-Ci`, not the later `-i`.
            rules("\t$(MAKE) -k -Ci clean -i\n"),
            // `-i` must follow `make` and be its own flag.
            rules("\tmake foo-ib clean\n"),
        ];
        let fail = vec![
            rules("\t-$(MAKE) distclean\n"),
            rules("\t-${MAKE} clean\n"),
            rules("\t-make clean\n"),
            rules("\t-$(MAKE) -C src clean\n"),
            rules("\tmake -i clean\n"),
            rules("\t$(MAKE) -i distclean\n"),
            rules("\t$(MAKE) -ki clean\n"),
            rules("\t$(MAKE) -C src -i clean\n"),
            // Continuations join into one line.
            rules("\tmake -i \\\n\t  clean\n"),
            // One report per line.
            rules("\tmake -i clean\n\t-make distclean\n"),
        ];
        SourceTreeTester::new(DebianRulesIgnoresMakeCleanError, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("\t$(MAKE) clean\n"))];
        let fail = vec![SourcePackageCase::from(rules("\t-$(MAKE) distclean\n"))];
        SourcePackageTester::new(DebianRulesIgnoresMakeCleanError, pass, fail).test_and_snapshot();
    }
}
