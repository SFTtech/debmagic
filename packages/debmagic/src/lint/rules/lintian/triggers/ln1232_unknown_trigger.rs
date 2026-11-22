use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::data;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct UnknownTrigger;

declare_rule! {
    /// The `triggers` control file has a directive whose type is not in lintian's
    /// `triggers/trigger-types` table (vendored from the pinned lintian), so dpkg
    /// may not support it. Extra is the unknown type; the line is the Location
    /// (the control file is `triggers`). Udebs are checked too.
    ///
    /// Trimmed lines are skipped when blank or starting with `#`. A directive is
    /// split on its first single space into a type and a name; a line with no
    /// name is not a directive. Line numbers count every line of the file.
    UnknownTrigger,
    code = "LN1232",
    tag = "unknown-trigger",
    default_selected = true,
    experimental = false,
}

/// Trigger types lintian knows, the keys of `triggers/trigger-types`.
static KNOWN_TYPES: LazyLock<BTreeSet<&'static str>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/triggers/trigger-types"))
        .filter_map(|line| line.split_once("=>").map(|(key, _)| key.trim()))
        .collect()
});

impl BinaryPackageRule for UnknownTrigger {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(text) = control_text(ctx, "triggers") else {
            return;
        };
        for (index, line) in text.split('\n').enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.splitn(2, ' ');
            let (Some(trigger_type), Some(name)) = (parts.next(), parts.next()) else {
                continue;
            };
            if trigger_type.is_empty() || name.is_empty() || KNOWN_TYPES.contains(trigger_type) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error(trigger_type).with_location(Location {
                path: PathBuf::from("triggers"),
                line: Some((index + 1) as u32),
                column: None,
            }));
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
    fn known_types_come_from_the_vendored_table() {
        for known in [
            "activate",
            "activate-await",
            "activate-noawait",
            "interest",
            "interest-await",
            "interest-noawait",
        ] {
            assert!(KNOWN_TYPES.contains(known), "{known}");
        }
        assert!(!KNOWN_TYPES.contains("random-trigger"));
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            triggers(""),
            triggers("interest /usr/share/foo\nactivate-noawait /usr/share/bar\n"),
            triggers("# random-trigger /x\n\ninterest-await /usr/share/foo2\n"),
            // A line with no name is not a directive.
            triggers("random-trigger\n"),
        ];
        let fail = vec![
            triggers("random-trigger /bar\n"),
            // Line numbers count every line of the file.
            triggers("# c\n\ninterest /x\nrandom-trigger /y\n"),
            triggers("random-trigger /bar\n").udeb(),
        ];
        BinaryPackageTester::new(UnknownTrigger, pass, fail).test_and_snapshot();
    }
}
