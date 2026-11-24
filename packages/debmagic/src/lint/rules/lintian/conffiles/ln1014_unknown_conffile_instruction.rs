use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::ln1012_relative_conffile::{conffiles_text, parse};

const KNOWN_INSTRUCTIONS: &[&str] = &["remove-on-upgrade"];

pub struct UnknownConffileInstruction;

declare_rule! {
    /// A `conffiles` entry has an instruction other than `remove-on-upgrade`.
    ///
    /// Each line's words before the path are instructions. Every distinct
    /// unknown instruction on a line is reported once, in sorted order. Extra
    /// is the instruction. Location is `conffiles` at that line. A non-UTF-8
    /// member is skipped. Udebs are checked too.
    UnknownConffileInstruction,
    code = "LN1014",
    tag = "unknown-conffile-instruction",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnknownConffileInstruction {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(text) = conffiles_text(ctx) else {
            return;
        };
        for entry in parse(&text) {
            let unknown: BTreeSet<&str> = entry
                .instructions
                .iter()
                .map(String::as_str)
                .filter(|instruction| !KNOWN_INSTRUCTIONS.contains(instruction))
                .collect();
            for instruction in unknown {
                ctx.diagnostic(Diagnostic::error(instruction).with_location(Location {
                    path: PathBuf::from("conffiles"),
                    line: Some(entry.line),
                    column: None,
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn conffiles(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("conffiles", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            conffiles(""),
            conffiles("/etc/foo\n"),
            conffiles("remove-on-upgrade /etc/foo\n"),
            conffiles("remove-on-upgrade remove-on-upgrade /etc/foo\n"),
        ];
        let fail = vec![
            conffiles("chicken-soup /etc/some.config\n"),
            conffiles("/etc/foo\n\nzeta alpha remove-on-upgrade /etc/bar\n"),
            conffiles("chicken-soup chicken-soup /etc/foo\n").udeb(),
            conffiles("chicken-soup  /etc/foo  \n"),
        ];
        BinaryPackageTester::new(UnknownConffileInstruction, pass, fail).test_and_snapshot();
    }
}
