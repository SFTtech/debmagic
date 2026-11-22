use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::text::{complete_lines, read_rules_from_tree, rules_location};

pub struct DebianRulesParsesDpkgParsechangelog;

declare_rule! {
    /// `debian/rules` parses the output of `dpkg-parsechangelog` for the
    /// version, source, date, or timestamp.
    ///
    /// Reads the logical lines of `debian/rules` (comments dropped,
    /// `\` continuations joined). A line that mentions `dpkg-parsechangelog`
    /// followed later on the line by `Source`, `Version`, `Date`, or
    /// `Timestamp` is reported, unless an `include` of a makefile lintian does
    /// not know came before it. A `.PHONY` line is not checked. Extra is empty;
    /// the Location is the last physical line of the logical line. Info, not
    /// selected by default.
    DebianRulesParsesDpkgParsechangelog,
    code = "LN1328",
    tag = "debian-rules-parses-dpkg-parsechangelog",
    default_selected = false,
    experimental = false,
}

static PARSE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)dpkg-parsechangelog.*(?:Source|Version|Date|Timestamp)").expect("regex")
});
static INCLUDE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[s-]?include\s+(\S+)").expect("regex"));
static PHONY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^(?:[^:]+\s)?\.PHONY(?:\s[^:]+)?:(.+)").expect("regex"));
static KNOWN_MAKEFILES: LazyLock<Vec<String>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/rules/known-makefiles"))
        .filter_map(|line| line.split_once("||").map(|(path, _)| path.to_string()))
        .collect()
});

impl SourceRule for DebianRulesParsesDpkgParsechangelog {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        let mut includes = false;
        let mut hits = Vec::new();
        for (number, line) in complete_lines(&contents) {
            if let Some(captures) = INCLUDE.captures(&line)
                && !KNOWN_MAKEFILES.iter().any(|path| path == &captures[1])
            {
                includes = true;
            }
            if PHONY.is_match(&line) {
                continue;
            }
            if !includes && PARSE.is_match(&line) {
                hits.push(number);
            }
        }
        for number in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(rules_location(number)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn rules(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/rules",
            TestFile::new(format!("#!/usr/bin/make -f\n{body}")),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            rules("\n%:\n\tdh $@\n"),
            // A comment does not count.
            rules("\n# dpkg-parsechangelog -SVersion\n%:\n\tdh $@\n"),
            // The include is not known, so the later line is not checked.
            rules("\ninclude debian/rules.mk\n\n.PHONY: x\n\tdpkg-parsechangelog -SVersion\n"),
            // Without Source, Version, Date, or Timestamp.
            rules("\n%:\n\tdpkg-parsechangelog -l debian/changelog\n"),
            // A `.PHONY` line is skipped.
            rules("\n.PHONY: dpkg-parsechangelog -SVersion\n"),
        ];
        let fail = vec![
            rules("\nVERSION := $(shell dpkg-parsechangelog -SVersion)\n"),
            rules("\nDATE := $(shell dpkg-parsechangelog -SDate)\n"),
            rules("\n%:\n\tdpkg-parsechangelog -SSource \\\n\t\t| cat\n"),
            rules("\ninclude /usr/share/quilt/quilt.make\n%:\n\tdpkg-parsechangelog -STimestamp\n"),
        ];
        SourceTreeTester::new(DebianRulesParsesDpkgParsechangelog, pass, fail).test_and_snapshot();
    }
}
