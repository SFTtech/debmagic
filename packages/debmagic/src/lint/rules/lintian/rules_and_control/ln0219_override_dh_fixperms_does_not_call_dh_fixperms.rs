use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::rules_text::{read_rules, rules_lines};

pub struct OverrideDhFixpermsDoesNotCallDhFixperms;

declare_rule! {
    /// An `override_dh_fixperms` target never calls `dh_fixperms`.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined). `override_dh_fixperms`,
    /// `override_dh_fixperms-indep`, and `override_dh_fixperms-arch` are
    /// remembered at the physical line where the target line ends. A later
    /// definition of the same target updates that line and keeps the earlier
    /// recipe. A recipe line (indented, not a comment) counts as the call when
    /// it starts with a tab, optional whitespace, an optional `-`, then
    /// `dh_fixperms` with the same suffix, `$(overridden_command)`, or
    /// `$(DH_OVERRIDDEN_COMMAND)`. `define` ends the current target.
    /// `override_dh_clean` is a different Tag. Extra is empty. Applies to the
    /// Source tree and Source package. A missing, unreadable, or non-UTF-8
    /// `debian/rules` is not scanned.
    OverrideDhFixpermsDoesNotCallDhFixperms,
    code = "LN0219",
    tag = "override_dh_fixperms-does-not-call-dh_fixperms",
    default_selected = true,
    experimental = false,
}

static CONDITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ifn?(?:eq|def)\s").expect("valid regex"));
static TARGET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^\s:][^:]*):+").expect("valid regex"));

const SUFFIXES: [&str; 3] = ["", "-indep", "-arch"];

impl SourceRule for OverrideDhFixpermsDoesNotCallDhFixperms {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules) else {
            return;
        };
        for (number, _suffix) in missing_calls(&contents) {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/rules"),
                line: Some(number),
                column: None,
            }));
        }
    }
}

struct Tracked {
    line: u32,
    calls: bool,
    seen: bool,
}

fn missing_calls(contents: &str) -> Vec<(u32, &'static str)> {
    let mut tracked = SUFFIXES.map(|_| Tracked {
        line: 0,
        calls: false,
        seen: false,
    });
    let mut current: Vec<String> = Vec::new();
    for (number, line) in rules_lines(contents) {
        if !CONDITIONAL.is_match(&line)
            && let Some(found) = TARGET.captures(&line)
        {
            current = found[1].split_whitespace().map(str::to_string).collect();
            for target in &current {
                if let Some(index) = suffix_index(target) {
                    tracked[index].seen = true;
                    tracked[index].line = number;
                }
            }
        } else if line.starts_with("define ") {
            current.clear();
        } else if is_recipe(&line) {
            for target in &current {
                if let Some(index) = suffix_index(target) {
                    let command = format!("dh_fixperms{}", SUFFIXES[index]);
                    if recipe_calls(&line, &command) {
                        tracked[index].calls = true;
                    }
                }
            }
        }
    }
    SUFFIXES
        .into_iter()
        .enumerate()
        .filter(|(index, _)| tracked[*index].seen && !tracked[*index].calls)
        .map(|(index, suffix)| (tracked[index].line, suffix))
        .collect()
}

fn suffix_index(target: &str) -> Option<usize> {
    let rest = target.strip_prefix("override_")?;
    SUFFIXES
        .iter()
        .position(|suffix| rest == format!("dh_fixperms{suffix}"))
}

fn is_recipe(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.len() < line.len() && !trimmed.is_empty() && !trimmed.starts_with('#')
}

fn recipe_calls(line: &str, command: &str) -> bool {
    let Some(rest) = line.strip_prefix('\t') else {
        return false;
    };
    let rest = rest.trim_start_matches([' ', '\t', '\n', '\r', '\u{000c}']);
    let rest = rest.strip_prefix('-').unwrap_or(rest);
    if rest.starts_with("$(overridden_command)") || rest.starts_with("$(DH_OVERRIDDEN_COMMAND)") {
        return true;
    }
    let Some(after) = rest.strip_prefix(command) else {
        return false;
    };
    after
        .chars()
        .next()
        .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    fn package(body: &str) -> Vec<(&'static str, TestFile)> {
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
            package(""),
            package("override_dh_fixperms:\n\tdh_fixperms\n"),
            package("override_dh_fixperms:\n\t-dh_fixperms\n"),
            package("override_dh_fixperms:\n\t$(overridden_command)\n"),
            package("override_dh_fixperms-indep:\n\t$(DH_OVERRIDDEN_COMMAND)\n"),
            package("override_dh_fixperms-arch:\n\tdh_fixperms-arch\n"),
            package("override_dh_fixperms-indep:\n\tdh_fixperms-indep --foo\n"),
            // A different override.
            package("override_dh_clean:\n"),
            package("override_dh_clean:\n\ttrue\n"),
            // The call is inside a conditional, and a continued recipe still calls.
            package("override_dh_fixperms:\nifeq ($(foo),bar)\n\tdh_fixperms\nendif\n"),
            package("override_dh_fixperms:\n\tdh_fixperms \\\n\t\t--foo\n"),
            // A comment is not the body, but a later real call is.
            package("override_dh_fixperms:\n\t# dh_fixperms\n\tdh_fixperms\n"),
        ];
        let fail = vec![
            package("override_dh_fixperms:\n"),
            package(
                "override_dh_fixperms:\noverride_dh_fixperms-indep:\noverride_dh_fixperms-arch:\n",
            ),
            package("override_dh_fixperms:\n\ttrue\n"),
            package("override_dh_fixperms:\n\techo dh_fixperms\n"),
            // Spaces are not a recipe tab.
            package("override_dh_fixperms:\n    dh_fixperms\n"),
            package("override_dh_fixperms:\n\t# dh_fixperms\n"),
            // `define` ends the target, so the later call belongs to nobody.
            package("override_dh_fixperms:\ndefine something\n\tdh_fixperms\n"),
            // Shared recipe: the suffix that was not called is reported.
            package("override_dh_fixperms override_dh_fixperms-arch:\n\tdh_fixperms\n"),
            package("override_dh_fixperms:\n\ttrue\noverride_dh_fixperms:\n\ttrue\n"),
        ];
        SourceTreeTester::new(OverrideDhFixpermsDoesNotCallDhFixperms, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            "override_dh_fixperms:\n\tdh_fixperms\n",
        ))];
        let fail = vec![SourcePackageCase::from(package("override_dh_fixperms:\n"))];
        SourcePackageTester::new(OverrideDhFixpermsDoesNotCallDhFixperms, pass, fail)
            .test_and_snapshot();
    }
}
