use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::declared::declared;

pub struct DuplicateConffile;

declare_rule! {
    /// A path is listed more than once in `conffiles`.
    ///
    /// Lines are grouped by path (leading `/` removed). Each path with more
    /// than one line is reported once, in path order. Extra is the path and
    /// `(lines N M ...)` with its line numbers ascending. Location is
    /// `conffiles` without a line. Udebs are checked too.
    DuplicateConffile,
    code = "LN0778",
    tag = "duplicate-conffile",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DuplicateConffile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let mut lines: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for entry in declared(ctx) {
            lines.entry(entry.path).or_default().push(entry.line);
        }
        for (path, numbers) in lines {
            if numbers.len() < 2 {
                continue;
            }
            let numbers: Vec<String> = numbers.iter().map(u32::to_string).collect();
            ctx.diagnostic(
                Diagnostic::error(format!("{path} (lines {})", numbers.join(" "))).with_location(
                    Location {
                        path: PathBuf::from("conffiles"),
                        line: None,
                        column: None,
                    },
                ),
            );
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
            conffiles("/etc/foo\n/etc/bar\n"),
            conffiles("/etc/foo\n/etc/foo.conf\n"),
            conffiles("remove-on-upgrade /etc/foo\n\n/etc/bar\n"),
            conffiles("/etc/foo\n\n//etc/foo\n"),
        ];
        let fail = vec![
            conffiles("/etc/foo\netc/foo\n"),
            conffiles("/etc/foo\n/etc/bar\n/etc/foo\n/etc/foo\n"),
            conffiles("/etc/foo\n/etc/foo\n").udeb(),
            conffiles("remove-on-upgrade /etc/foo\n/etc/foo\n"),
        ];
        BinaryPackageTester::new(DuplicateConffile, pass, fail).test_and_snapshot();
    }
}
