use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree};

pub struct OverrideDhCleanDoesNotCallDhClean;

declare_rule! {
    /// An `override_dh_clean` target never calls `dh_clean`.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined). `override_dh_clean`, `override_dh_clean-indep`,
    /// and `override_dh_clean-arch` are remembered at the physical line where
    /// the target line ends; a later definition of the same target moves that
    /// line and keeps the earlier recipe lines. A recipe line (indented, not a
    /// comment) calls `dh_clean` when it starts with a tab, optional
    /// whitespace, an optional `-`, then `dh_clean` at a word boundary,
    /// `$(overridden_command)`, or `$(DH_OVERRIDDEN_COMMAND)`. Any of the three
    /// targets counts for any suffix: lintian checks for plain `dh_clean`.
    /// A `define` ends the current target, and a `.PHONY` line is not a
    /// target. Extra is empty. Location is `debian/rules` at the target line.
    /// Applies to the Source tree and Source package. A missing, unreadable,
    /// or non-UTF-8 `debian/rules` is not scanned. A symlinked `debian/rules`
    /// is not followed for its contents (lintian follows it).
    OverrideDhCleanDoesNotCallDhClean,
    code = "LN0915",
    tag = "override_dh_clean-does-not-call-dh_clean",
    default_selected = true,
    experimental = false,
}

static CONDITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ifn?(?:eq|def)\s").expect("valid regex"));
static TARGET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^([^\s:][^:]*):+(.*)").expect("valid regex"));
static PHONY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^(?:[^:]+\s)?\.PHONY(?:\s[^:]+)?:(.+)").expect("valid regex")
});
static RECIPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+[^#]").expect("valid regex"));
static DH_CLEAN_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\t\s*-?(dh_clean\b|\$\(overridden_command\)|\$\(DH_OVERRIDDEN_COMMAND\))")
        .expect("valid regex")
});

/// The suffixes lintian checks, in its order.
const SUFFIXES: [&str; 3] = ["", "-indep", "-arch"];

/// One `override_dh_clean` target: the physical line of its last definition,
/// whether it was seen, and whether a recipe line calls `dh_clean`.
#[derive(Clone, Copy, Default)]
struct Tracked {
    line: u32,
    seen: bool,
    calls: bool,
}

impl SourceRule for OverrideDhCleanDoesNotCallDhClean {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for line in missing_calls(&contents) {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/rules"),
                line: Some(line),
                column: None,
            }));
        }
    }
}

/// Target lines of the suffixed `override_dh_clean` targets that no recipe
/// line calls `dh_clean` for, in suffix order.
fn missing_calls(contents: &str) -> Vec<u32> {
    let mut tracked = [Tracked::default(); SUFFIXES.len()];
    let mut current: Vec<String> = Vec::new();
    for (number, line) in complete_lines(contents) {
        // A `.PHONY` line does not change the current target.
        if PHONY.is_match(&line) {
            continue;
        }
        if !CONDITIONAL.is_match(&line)
            && let Some(found) = TARGET.captures(&line)
        {
            current = found[1].split_whitespace().map(str::to_string).collect();
            for target in &current {
                // lintian's `override_(.+)`: the first `override_`, then a
                // non-empty rest, which keys the command it is checked against.
                let Some(rest) = override_rest(target) else {
                    continue;
                };
                if let Some(index) = suffix_index(rest) {
                    tracked[index].seen = true;
                    tracked[index].line = number;
                }
            }
        } else if line.starts_with("define ") {
            current.clear();
        } else if RECIPE.is_match(&line) {
            for target in &current {
                if let Some(index) = SUFFIXES
                    .iter()
                    .position(|suffix| target == &format!("override_dh_clean{suffix}"))
                    && DH_CLEAN_CALL.is_match(&line)
                {
                    tracked[index].calls = true;
                }
            }
        }
    }
    tracked
        .iter()
        .filter(|entry| entry.seen && !entry.calls)
        .map(|entry| entry.line)
        .collect()
}

/// The text after the first `override_` in a target name, when it is not empty.
fn override_rest(target: &str) -> Option<&str> {
    let (_, rest) = target.split_once("override_")?;
    (!rest.is_empty()).then_some(rest)
}

/// The index in [`SUFFIXES`] of `dh_clean` plus a suffix, for the rest of an
/// `override_` target.
fn suffix_index(rest: &str) -> Option<usize> {
    let suffix = rest.strip_prefix("dh_clean")?;
    SUFFIXES.iter().position(|known| *known == suffix)
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
            rules("override_dh_clean:\n\tdh_clean\n"),
            rules("override_dh_clean:\n\t-dh_clean\n"),
            rules("override_dh_clean:\n\t$(overridden_command)\n"),
            rules("override_dh_clean:\n\t$(DH_OVERRIDDEN_COMMAND) -x\n"),
            // Extra indentation is still a recipe.
            rules("override_dh_clean:\n\t\tdh_clean\n"),
            // Any of the three targets may call the plain command.
            rules("override_dh_clean-indep:\n\tdh_clean\n"),
            rules("override_dh_clean-arch:\n\tdh_clean -X\n"),
            // A second definition keeps the first one's recipe.
            rules("override_dh_clean:\n\tdh_clean\noverride_dh_clean:\n\trm x\n"),
            // A target that is not an override is not checked.
            rules("clean:\n\trm x\n"),
        ];
        let fail = vec![
            rules("override_dh_clean:\n\trm x\n"),
            // `dh_clean` must end at a word boundary.
            rules("override_dh_clean:\n\t-dh_clean_extra\n"),
            // An empty override calls nothing.
            rules("override_dh_clean:\n"),
            // A comment does not call anything.
            rules("override_dh_clean:\n\t# dh_clean\n"),
            // The dash must be right before the command.
            rules("override_dh_clean:\n\t- dh_clean\n"),
            rules("override_dh_clean-arch:\n\trm x\n"),
            // Each suffix is judged on its own: the `-indep` target lacks the call.
            rules("override_dh_clean:\n\tdh_clean\noverride_dh_clean-indep:\n\trm x\n"),
        ];
        SourceTreeTester::new(OverrideDhCleanDoesNotCallDhClean, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "override_dh_clean:\n\tdh_clean\n",
        ))];
        let fail = vec![SourcePackageCase::from(rules(
            "override_dh_clean:\n\trm x\n",
        ))];
        SourcePackageTester::new(OverrideDhCleanDoesNotCallDhClean, pass, fail).test_and_snapshot();
    }
}
