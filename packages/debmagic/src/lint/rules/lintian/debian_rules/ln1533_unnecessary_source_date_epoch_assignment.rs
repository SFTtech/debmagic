use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::text::{complete_lines, read_rules_from_tree};

pub struct UnnecessarySourceDateEpochAssignment;

declare_rule! {
    /// `debian/rules` sets `SOURCE_DATE_EPOCH` although the package does not
    /// need it.
    ///
    /// Walks the logical lines of `debian/rules` and takes the variable name of
    /// each assignment, the last word before `=`, `:=`, `?=` or `+=`. Fires for
    /// each `SOURCE_DATE_EPOCH` assignment unless Build-Depends, Build-Depends-
    /// Indep or Build-Depends-Arch satisfies `dpkg-dev (>= 1.18.8) | debhelper
    /// (>= 10.10)`. Each assignment gets a Diagnostic with the file as Location,
    /// at the line where the logical line ends. Info, not selected by default.
    /// Assignments inside conditionals count, as in lintian.
    UnnecessarySourceDateEpochAssignment,
    code = "LN1533",
    tag = "unnecessary-source-date-epoch-assignment",
    default_selected = false,
    experimental = false,
}

/// Lintian's `Build-Depends-All`.
const BUILD_DEPENDS_ALL: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

const SUPPORTED: &str = "dpkg-dev (>= 1.18.8) | debhelper (>= 10.10)";

static ASSIGNMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^\s*(?:\S+\s+)*?(\S+)\s*[:?+]?=\s*(.*)$").expect("regex"));

impl SourceRule for UnnecessarySourceDateEpochAssignment {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        let supported = ctx.debian_control().is_some_and(|control| {
            control
                .source()
                .relations_and(BUILD_DEPENDS_ALL)
                .satisfies(SUPPORTED)
        });
        if supported {
            return;
        }
        for (line_number, line) in complete_lines(&contents) {
            let Some(captures) = ASSIGNMENT.captures(&line) else {
                continue;
            };
            if &captures[1] == "SOURCE_DATE_EPOCH" {
                ctx.diagnostic(
                    Diagnostic::warn("")
                        .with_severity(Severity::Info)
                        .with_location(Location {
                            path: PathBuf::from("debian/rules"),
                            line: Some(line_number),
                            column: None,
                        }),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn rules(build_depends: &str, contents: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: example\nMaintainer: Example <ex@example.com>\nBuild-Depends: {build_depends}\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
                )),
            ),
            ("debian/rules", TestFile::new(contents)),
        ]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            rules("debhelper (>= 10.10)", "SOURCE_DATE_EPOCH := 1\n"),
            rules(
                "dpkg-dev (>= 1.18.8), debhelper",
                "export SOURCE_DATE_EPOCH=1\n",
            ),
            // Other variables and names that only contain the variable name do not count.
            rules(
                "debhelper",
                "OTHER_SOURCE_DATE_EPOCH = 1\nSOURCE_DATE_EPOCH_X = 1\n",
            ),
            // A comment is not an assignment.
            rules("debhelper", "# SOURCE_DATE_EPOCH = 1\n"),
            // A lone prerequisite line is not an assignment.
            rules("debhelper", "build: SOURCE_DATE_EPOCH\n"),
        ];
        let fail = vec![
            rules("debhelper", "SOURCE_DATE_EPOCH := 1\n"),
            rules(
                "debhelper (>= 9)",
                "export SOURCE_DATE_EPOCH ?= $(shell date)\n",
            ),
            // Inside a conditional it still counts.
            rules("debhelper", "ifeq (a,b)\nSOURCE_DATE_EPOCH = 1\nendif\n"),
            // Continued lines are reported at their last physical line.
            rules("debhelper", "SOURCE_DATE_EPOCH = \\\n  1\n"),
        ];
        SourceTreeTester::new(UnnecessarySourceDateEpochAssignment, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "debhelper (>= 10.10)",
            "SOURCE_DATE_EPOCH = 1\n",
        ))];
        let fail = vec![SourcePackageCase::from(rules(
            "debhelper",
            "SOURCE_DATE_EPOCH = 1\n",
        ))];
        SourcePackageTester::new(UnnecessarySourceDateEpochAssignment, pass, fail)
            .test_and_snapshot();
    }
}
