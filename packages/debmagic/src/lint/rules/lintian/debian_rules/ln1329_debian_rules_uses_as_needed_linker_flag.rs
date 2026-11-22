use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::text::{complete_lines, read_rules_from_tree, rules_location};

pub struct DebianRulesUsesAsNeededLinkerFlag;

declare_rule! {
    /// `debian/rules` passes the `--as-needed` linker flag.
    ///
    /// Reads the logical lines of `debian/rules` (comments dropped,
    /// `\` continuations joined). A line that contains `--as-needed` but not
    /// `--no-as-needed` is reported at its last physical line. Extra is empty.
    /// Since Bullseye the flag is the default, so it is not needed. Pedantic,
    /// selected only when asked for.
    DebianRulesUsesAsNeededLinkerFlag,
    code = "LN1329",
    tag = "debian-rules-uses-as-needed-linker-flag",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DebianRulesUsesAsNeededLinkerFlag {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            if line.contains("--as-needed") && !line.contains("--no-as-needed") {
                ctx.diagnostic(
                    Diagnostic::warn("")
                        .with_severity(Severity::Pedantic)
                        .with_location(rules_location(number)),
                );
            }
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
            rules("\nLDFLAGS += -Wl,--no-as-needed\n"),
            // A comment does not count.
            rules("\n# -Wl,--as-needed\n%:\n\tdh $@\n"),
        ];
        let fail = vec![
            rules("\nLDFLAGS += -Wl,--as-needed\n"),
            rules("\n%:\n\tdh $@ --as-needed \\\n\t\t-Wl,--as-needed\n"),
        ];
        SourceTreeTester::new(DebianRulesUsesAsNeededLinkerFlag, pass, fail).test_and_snapshot();
    }
}
