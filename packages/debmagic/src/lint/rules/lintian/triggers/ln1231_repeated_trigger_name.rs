use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct RepeatedTriggerName;

declare_rule! {
    /// The `triggers` control file declares the same trigger name more than once,
    /// whatever the trigger types. Extra is the name and the lines it is declared
    /// on, as lintian words it: `NAME (lines 14 15)`. The control file is the
    /// Location (no line). Udebs are checked too.
    ///
    /// Trimmed lines are skipped when blank or starting with `#`. A directive is
    /// split on its first single space into a type and a name; a line with no
    /// name does not count. Line numbers count every line of the file. Repeated
    /// names are reported in name order.
    RepeatedTriggerName,
    code = "LN1231",
    tag = "repeated-trigger-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for RepeatedTriggerName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(text) = control_text(ctx, "triggers") else {
            return;
        };
        let mut lines_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, line) in text.split('\n').enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.splitn(2, ' ');
            let (Some(trigger_type), Some(name)) = (parts.next(), parts.next()) else {
                continue;
            };
            if trigger_type.is_empty() || name.is_empty() {
                continue;
            }
            lines_by_name
                .entry(name.to_string())
                .or_default()
                .push(index + 1);
        }
        for (name, lines) in lines_by_name {
            if lines.len() < 2 {
                continue;
            }
            let numbers: Vec<String> = lines.iter().map(usize::to_string).collect();
            ctx.diagnostic(
                Diagnostic::error(format!("{name} (lines {})", numbers.join(" "))).with_location(
                    Location {
                        path: PathBuf::from("triggers"),
                        line: None,
                        column: None,
                    },
                ),
            );
        }
    }
}

fn control_text(ctx: &mut BinaryPackageContext<'_>, name: &str) -> Option<String> {
    let files = ctx.control_files()?;
    let entry = files.get(name)?;
    let bytes = files.read(entry).ok()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn triggers(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("triggers", TestFile::new(body))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            triggers(""),
            triggers("interest /usr/share/foo\ninterest /usr/share/bar\n"),
            triggers("# interest /usr/share/foo\n\ninterest /usr/share/foo2\n"),
            // A line with no name is not a directive.
            triggers("interest\ninterest\n"),
            // Same name, different text: not a repeat.
            triggers("interest /usr/share/foo\ninterest /usr/share/foo/\n"),
        ];
        let fail = vec![
            triggers("interest-await /usr/share/foo2\ninterest-noawait /usr/share/foo2\n"),
            // Any types count; line numbers include skipped lines.
            triggers("# c\n\nactivate /x\nrandom-trigger /y\n\nactivate-noawait /x\n"),
            // Two repeated names, reported in name order.
            triggers("interest /b\ninterest /a\ninterest /b\ninterest /a\ninterest /a\n"),
            triggers("interest /usr/share/foo\ninterest /usr/share/foo\n").udeb(),
        ];
        BinaryPackageTester::new(RepeatedTriggerName, pass, fail).test_and_snapshot();
    }
}
