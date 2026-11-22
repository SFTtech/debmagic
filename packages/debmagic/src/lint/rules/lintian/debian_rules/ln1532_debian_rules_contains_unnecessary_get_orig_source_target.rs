use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::text::{complete_lines, read_rules_from_tree};

pub struct DebianRulesContainsUnnecessaryGetOrigSourceTarget;

declare_rule! {
    /// `debian/rules` has a `get-orig-source` target whose body runs `uscan`.
    ///
    /// Walks the logical lines of `debian/rules` as lintian does: a target line
    /// (a line before a colon that does not start with `ifeq`, `ifneq`,
    /// `ifdef` or `ifndef`) makes its names the current targets, `define`
    /// clears them, and an indented line is a body line of every current
    /// target. Fires once, with the file as Location, when a body line of the
    /// `get-orig-source` target starts with `uscan` as a word. Info, not
    /// selected by default. `.PHONY` lines do not change the current targets.
    /// Variable expansion of target names is not followed.
    DebianRulesContainsUnnecessaryGetOrigSourceTarget,
    code = "LN1532",
    tag = "debian-rules-contains-unnecessary-get-orig-source-target",
    default_selected = false,
    experimental = false,
}

static PHONY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[^:]+\s)?\.PHONY(?:\s[^:]+)?:(.+)").expect("regex"));
static CONDITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ifn?(?:eq|def)\s").expect("regex"));
static TARGET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([^\s:][^:]*):+").expect("regex"));
static BODY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+[^#]").expect("regex"));
static USCAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+uscan\b").expect("regex"));

impl SourceRule for DebianRulesContainsUnnecessaryGetOrigSourceTarget {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        if get_orig_source_runs_uscan(&contents) {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: PathBuf::from("debian/rules"),
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// Whether a body line of the `get-orig-source` target starts with `uscan`.
fn get_orig_source_runs_uscan(contents: &str) -> bool {
    let mut current: Vec<String> = Vec::new();
    let mut body: HashMap<String, Vec<String>> = HashMap::new();

    for (_, line) in complete_lines(contents) {
        if PHONY.is_match(&line) {
            continue;
        }
        if !CONDITIONAL.is_match(&line)
            && let Some(names) = target_names(&line)
        {
            current = names;
            continue;
        }
        if line.starts_with("define ") {
            current.clear();
        } else if BODY.is_match(&line) {
            for target in &current {
                body.entry(target.clone()).or_default().push(line.clone());
            }
        }
    }

    body.get("get-orig-source")
        .is_some_and(|lines| lines.iter().any(|line| USCAN.is_match(line)))
}

/// The space-separated names before the first colon of a target line.
fn target_names(line: &str) -> Option<Vec<String>> {
    let captures = TARGET.captures(line)?;
    Some(captures[1].split_whitespace().map(str::to_string).collect())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn rules(contents: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/rules", TestFile::new(contents))]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            rules("#!/usr/bin/make -f\nget-orig-source:\n\tcurl -O https://example.org/x\n"),
            // uscan under another target is not the get-orig-source target.
            rules("build:\n\tuscan --force-download\nget-orig-source:\n\t@echo hi\n"),
            // Commented-out uscan does not count.
            rules("get-orig-source:\n\t# uscan\n\t#uscan\n"),
            // uscan must start the command, not appear later in it.
            rules("get-orig-source:\n\techo uscan\n"),
            // A define clears the current targets.
            rules("get-orig-source:\ndefine x\n\tuscan\nendef\n"),
            // A .PHONY line does not make the next body line a body of its target.
            rules(".PHONY: get-orig-source\n\tuscan\n"),
        ];
        let fail = vec![
            rules("#!/usr/bin/make -f\nget-orig-source:\n\tuscan --force-download\n"),
            rules("get-orig-source:\n\t  uscan --x\n"),
            // Several targets in one rule.
            rules("build get-orig-source:\n\tuscan\n"),
            // A continued target line still names get-orig-source.
            rules("get-orig-source: \\\n\tbuild\n\tuscan\n"),
            // A conditional does not end the body of its target.
            rules("get-orig-source:\nifeq (a,b)\n\tuscan\nendif\n"),
        ];
        SourceTreeTester::new(
            DebianRulesContainsUnnecessaryGetOrigSourceTarget,
            pass,
            fail,
        )
        .test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(rules("build:\n\tdh build\n"))];
        let fail = vec![SourcePackageCase::from(rules(
            "get-orig-source:\n\tuscan\n",
        ))];
        SourcePackageTester::new(
            DebianRulesContainsUnnecessaryGetOrigSourceTarget,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
