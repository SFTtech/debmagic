use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::{ChangesRule, Severity};

use super::{bpo_numbers, changes_entry_count, digits_value};

pub struct BackportsChangesMissing;

declare_rule! {
    /// A backports upload (a `Distribution` target with `backports` in its
    /// name) whose version is `~bpoN+M` with `M` below 2, or has no such
    /// suffix, and whose `Changes` field has a single changelog entry. Such
    /// an upload is expected to carry the changes of several entries. Reported
    /// once per backports target, with no extra. Severity is info and the Tag
    /// is not selected by default, as in lintian.
    ///
    /// The entry count is lintian's: lines of the form `source (version)
    /// distribution` in the `Changes` field.
    BackportsChangesMissing,
    code = "LN1481",
    tag = "backports-changes-missing",
    default_selected = false,
    experimental = false,
}

impl ChangesRule for BackportsChangesMissing {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let changes = ctx.changes();
        let backports = changes
            .distributions()
            .into_iter()
            .filter(|target| *target != "UNRELEASED" && target.contains("backports"))
            .count();
        if backports == 0 {
            return;
        }
        let bpo_version = changes
            .version()
            .and_then(bpo_numbers)
            .map_or(0, |(_, bpo)| digits_value(bpo));
        let entries =
            changes_entry_count(changes.fields().untrimmed_value("Changes").unwrap_or(""));
        if bpo_version < 2 && entries == 1 {
            for _ in 0..backports {
                ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    /// A `.changes` with `entries` changelog entries and the given version and
    /// distributions.
    fn changes(version: &str, distribution: &str, entries: usize) -> ChangesCase {
        let mut text = format!(
            "Source: example\nVersion: {version}\nDistribution: {distribution}\nChanges:\n"
        );
        for n in 0..entries {
            text.push_str(&format!(
                " example ({version}) {distribution}; urgency=low\n .\n   * change {n}\n .\n"
            ));
        }
        ChangesCase::from(text)
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            // Not a backports upload.
            changes("1.0-1", "bookworm", 1),
            // Enough backports suffixes, or several entries.
            changes("1.0~bpo12+2", "bookworm-backports", 1),
            changes("1.0~bpo12+3+salsaci+1", "bookworm-backports", 1),
            changes("1.0~bpo12+1", "bookworm-backports", 2),
            changes("1.0~bpo12+1", "bookworm-backports bookworm", 2),
            changes("1.0~bpo12+1", "UNRELEASED", 1),
            // A large backports count is not below two.
            changes("1.0~bpo12+99999999999999999999999", "bookworm-backports", 1),
        ];
        let fail = vec![
            changes("1.0-1", "bookworm-backports", 1),
            changes("1.0~bpo12+1", "bookworm-backports", 1),
            changes("1.0~bpo12+1", "bookworm-backports bookworm-security", 1),
            changes("1.0~bpo12+1+salsaci", "bookworm-backports-sloppy", 1),
        ];
        ChangesTester::new(BackportsChangesMissing, pass, fail).test_and_snapshot();
    }
}
