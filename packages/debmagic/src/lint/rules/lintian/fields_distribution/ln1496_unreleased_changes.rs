use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

pub struct UnreleasedChanges;

declare_rule! {
    /// The `Changes` field of the `.changes` copied from the first entry of
    /// `debian/changelog` names `UNRELEASED` as its distribution, and the
    /// `Distribution` field of the `.changes` does not name `UNRELEASED`.
    /// The package was not meant for a release yet. The hint has an empty
    /// extra.
    ///
    /// The first entry is the first `name (version) words;` of the `Changes`
    /// field after its leading whitespace, as lintian reads it, and its words
    /// are split on whitespace. A `Changes` field with no such entry is not
    /// reported.
    UnreleasedChanges,
    code = "LN1496",
    tag = "unreleased-changes",
    default_selected = true,
    experimental = false,
}

/// lintian's first-entry pattern: the distribution words between the version
/// and the first `;`, which may run over lines.
fn first_entry_pattern() -> &'static Regex {
    static PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\S+\s+\([^()]+\)([^;]+);").expect("first entry regex"));
    &PATTERN
}

impl ChangesRule for UnreleasedChanges {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let changes = ctx.changes();
        let Some(text) = changes.fields().get("Changes") else {
            return;
        };
        let Some(captures) = first_entry_pattern().captures(text) else {
            return;
        };
        let entry_distributions: Vec<&str> = captures[1].split_whitespace().collect();
        if !entry_distributions.contains(&"UNRELEASED") {
            return;
        }
        if changes.distributions().contains(&"UNRELEASED") {
            return;
        }
        ctx.diagnostic(Diagnostic::error(""));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    fn changes(distribution: &str, entry: &str) -> ChangesCase {
        ChangesCase::from(format!(
            "Source: example\nVersion: 1.0\nDistribution: {distribution}\nChanges:\n example (1.0) {entry}; urgency=low\n .\n   * Change.\n"
        ))
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes("unstable", "unstable"),
            changes("UNRELEASED", "UNRELEASED"),
            changes("unstable UNRELEASED", "UNRELEASED"),
            // The entry names two distributions, one of them UNRELEASED: the
            // Distribution field names it too, so it is not reported.
            changes("UNRELEASED", "unstable UNRELEASED"),
            // No Changes field, or no entry in it.
            ChangesCase::from("Source: example\nDistribution: unstable\n"),
            ChangesCase::from("Source: example\nDistribution: unstable\nChanges:\n not an entry\n"),
            // The word is compared as written: lowercase is not UNRELEASED.
            changes("unstable", "unreleased"),
        ];
        let fail = vec![
            changes("unstable", "UNRELEASED"),
            changes("experimental", "UNRELEASED"),
            changes("unstable", "unstable UNRELEASED"),
            // A first entry that runs over lines.
            ChangesCase::from(
                "Source: example\nDistribution: unstable\nChanges:\n example (1.0)\n  UNRELEASED;\n urgency=low\n",
            ),
        ];
        ChangesTester::new(UnreleasedChanges, pass, fail).test_and_snapshot();
    }
}
