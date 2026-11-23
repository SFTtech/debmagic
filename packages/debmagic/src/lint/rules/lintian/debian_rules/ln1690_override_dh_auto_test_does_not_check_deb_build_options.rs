use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::text::{complete_lines, read_rules_from_tree};

pub struct OverrideDhAutoTestDoesNotCheckDEBBUILDOPTIONS;

declare_rule! {
    /// The `override_dh_auto_test` target of `debian/rules` does not check
    /// `DEB_BUILD_OPTIONS` (`nocheck`) itself, and the debhelper compat level
    /// is below 13 (or undeclared). Before compat 13, debhelper does not skip
    /// the tests for `nocheck`, so the override has to. Reported at info
    /// severity with an empty extra, at the physical line where the last
    /// `override_dh_auto_test` target line ends.
    ///
    /// The Rule follows lintian's `debian/rules` scan (see `text.rs`): recipe
    /// lines of the exact target count only when they do not start with a
    /// tab and `:`/`[`, are not blank, do not mention `dh_auto_test`, and are
    /// not one of the boilerplate commands (`cp`, `chmod`, `echo`, `ln`, `mv`,
    /// `mkdir`, `rm`, `test`, `true`, optionally with `-`, `@` or `/bin/`). The
    /// check is skipped when any `ifeq`/`ifdef`/`ifneq`/`ifndef` condition in
    /// the file mentions `DEB_BUILD_OPTIONS` or `nocheck`. Applies to the
    /// Source tree and Source package.
    OverrideDhAutoTestDoesNotCheckDEBBUILDOPTIONS,
    code = "LN1690",
    tag = "override_dh_auto_test-does-not-check-DEB_BUILD_OPTIONS",
    default_selected = false,
    experimental = false,
}

/// lintian's `$MIN_DH_COMPAT_DH_AUTO_TEST`.
const MIN_DH_COMPAT_DH_AUTO_TEST: u32 = 13;
const TARGET_NAME: &str = "override_dh_auto_test";

static CONDITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ifn?(?:eq|def)\s(.*)").expect("valid regex"));
static TARGET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^([^\s:][^:]*):+(.*)").expect("valid regex"));
static PHONY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^(?:[^:]+\s)?\.PHONY(?:\s[^:]+)?:(.+)").expect("valid regex")
});
static RECIPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+[^#]").expect("valid regex"));
static NOCHECK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"DEB_BUILD_OPTIONS|nocheck").expect("valid regex"));
static NOT_A_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\t\s*[:\[]").expect("valid regex"));
static BLANK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*$").expect("valid regex"));
static DH_AUTO_TEST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bdh_auto_test\b").expect("valid regex"));
static BOILERPLATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\t\s*[-@]?(?:(?:/usr)?/bin/)?(?:cp|chmod|echo|ln|mv|mkdir|rm|test|true)")
        .expect("valid regex")
});

/// What the `debian/rules` scan keeps for this Rule.
#[derive(Default)]
struct Scan {
    /// The physical line where the last `override_dh_auto_test` target ends.
    overridden: Option<u32>,
    /// The recipe lines of the `override_dh_auto_test` target.
    recipes: Vec<String>,
    /// The argument of every `ifeq`, `ifneq`, `ifdef` and `ifndef` line.
    conditionals: Vec<String>,
}

impl SourceRule for OverrideDhAutoTestDoesNotCheckDEBBUILDOPTIONS {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        let scan = scan(&contents);
        let Some(line) = scan.overridden else {
            return;
        };
        let compat = ctx.debhelper().and_then(|facts| facts.declared_level());
        if compat.is_some_and(|level| level >= MIN_DH_COMPAT_DH_AUTO_TEST) {
            return;
        }
        let needed: Vec<&String> = scan
            .recipes
            .iter()
            .filter(|recipe| {
                !NOT_A_COMMAND.is_match(recipe)
                    && !BLANK.is_match(recipe)
                    && !DH_AUTO_TEST.is_match(recipe)
                    && !BOILERPLATE.is_match(recipe)
            })
            .collect();
        if needed.is_empty() || scan.conditionals.iter().any(|c| NOCHECK.is_match(c)) {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn("")
                .with_severity(Severity::Info)
                .with_location(Location {
                    path: PathBuf::from("debian/rules"),
                    line: Some(line),
                    column: None,
                }),
        );
    }
}

fn scan(contents: &str) -> Scan {
    let mut scan = Scan::default();
    let mut current: Vec<String> = Vec::new();
    for (number, line) in complete_lines(contents) {
        // A `.PHONY` line does not change the current target.
        if PHONY.is_match(&line) {
            continue;
        }
        let conditional = CONDITIONAL.captures(&line);
        if let Some(captures) = &conditional {
            scan.conditionals.push(captures[1].to_string());
        }
        if conditional.is_none()
            && let Some(found) = TARGET.captures(&line)
        {
            current = found[1].split_whitespace().map(str::to_string).collect();
            for target in &current {
                // lintian's `override_(.+)`, keyed by the rest of the name.
                if target
                    .split_once("override_")
                    .is_some_and(|(_, rest)| !rest.is_empty() && rest == "dh_auto_test")
                {
                    scan.overridden = Some(number);
                }
            }
        } else if line.starts_with("define ") {
            current.clear();
        } else if RECIPE.is_match(&line) && current.iter().any(|t| t == TARGET_NAME) {
            scan.recipes.push(line);
        }
    }
    scan
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(build_depends: &str, rules: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: example\n\
                     Maintainer: Example <ex@example.com>\n\
                     Standards-Version: 4.7.2\n\
                     Build-Depends: {build_depends}\n\
                     \n\
                     Package: example\n\
                     Architecture: any\n\
                     Description: example\n extra\n"
                )),
            ),
            ("debian/rules", TestFile::new(rules)),
        ]
    }

    const DH: &str = "#!/usr/bin/make -f\n%:\n\tdh $@\n";

    #[test]
    fn test() {
        let pass = vec![
            // No override of the test target.
            tree("debhelper-compat (= 12)", DH),
            // The recipe calls dh_auto_test, or is boilerplate.
            tree(
                "debhelper-compat (= 12)",
                "#!/usr/bin/make -f\noverride_dh_auto_test:\n\tdh_auto_test -- -v\n",
            ),
            tree(
                "debhelper-compat (= 12)",
                "#!/usr/bin/make -f\noverride_dh_auto_test:\n\t@echo skipped\n",
            ),
            // A recipe line that is a condition-like command is not counted.
            tree(
                "debhelper-compat (= 12)",
                "#!/usr/bin/make -f\noverride_dh_auto_test:\n\t[ -z x ]\n",
            ),
            // The conditional checks DEB_BUILD_OPTIONS.
            tree(
                "debhelper-compat (= 12)",
                "#!/usr/bin/make -f\n\
                 ifeq (,$(filter nocheck,$(DEB_BUILD_OPTIONS)))\n\
                 override_dh_auto_test:\n\
                 \t./run-tests\n\
                 endif\n",
            ),
            // Compat 13 and later skip the tests for nocheck themselves.
            tree(
                "debhelper-compat (= 13)",
                "#!/usr/bin/make -f\noverride_dh_auto_test:\n\t./run-tests\n%:\n\tdh $@\n",
            ),
        ];
        let fail = vec![
            tree(
                "debhelper-compat (= 12)",
                "#!/usr/bin/make -f\noverride_dh_auto_test:\n\t./run-tests\n%:\n\tdh $@\n",
            ),
            // No compat level declared.
            tree(
                "debhelper",
                "#!/usr/bin/make -f\noverride_dh_auto_test:\n\t./run-tests\n%:\n\tdh $@\n",
            ),
            // A condition that does not mention nocheck does not count.
            tree(
                "debhelper-compat (= 12)",
                "#!/usr/bin/make -f\n\
                 ifeq (a,b)\n\
                 endif\n\
                 override_dh_auto_test:\n\
                 \t./run-tests\n%:\n\tdh $@\n",
            ),
        ];
        SourceTreeTester::new(OverrideDhAutoTestDoesNotCheckDEBBUILDOPTIONS, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("debhelper-compat (= 13)", DH))];
        let fail = vec![SourcePackageCase::from(tree(
            "debhelper-compat (= 12)",
            "#!/usr/bin/make -f\noverride_dh_auto_test:\n\t./run-tests\n%:\n\tdh $@\n",
        ))];
        SourcePackageTester::new(OverrideDhAutoTestDoesNotCheckDEBBUILDOPTIONS, pass, fail)
            .test_and_snapshot();
    }
}
