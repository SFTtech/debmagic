use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

pub struct BadUrgencyInChangesFile;

declare_rule! {
    /// The `Urgency` field of the `.changes` does not start with a known
    /// urgency. The first word of the value is lowercased and cut at its first
    /// space; the Rule reports unless that word contains `low`, `medium`,
    /// `high`, `critical` or `emergency` as a substring, as lintian's
    /// unanchored patterns do. So `lowest` passes and `urgent` fails. The
    /// extra is the value as written. A `.changes` without an `Urgency` field
    /// is not reported.
    BadUrgencyInChangesFile,
    code = "LN1485",
    tag = "bad-urgency-in-changes-file",
    default_selected = true,
    experimental = false,
}

const KNOWN_URGENCIES: [&str; 5] = ["low", "medium", "high", "critical", "emergency"];

impl ChangesRule for BadUrgencyInChangesFile {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let fields = ctx.changes().fields();
        if !fields.declares("Urgency") {
            return;
        }
        let urgency = fields.get("Urgency").unwrap_or("");
        let lowercase = urgency.to_lowercase();
        let first_word = lowercase.split(' ').next().unwrap_or("");
        if !KNOWN_URGENCIES
            .iter()
            .any(|known| first_word.contains(known))
        {
            ctx.diagnostic(Diagnostic::error(urgency));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    fn changes(urgency: &str) -> ChangesCase {
        ChangesCase::from(format!(
            "Source: example\nVersion: 1.0-1\nUrgency: {urgency}\n"
        ))
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            ChangesCase::from("Source: example\nVersion: 1.0-1\n"),
            changes("low"),
            changes("Medium"),
            changes("HIGH"),
            changes("critical"),
            changes("emergency"),
            changes("high, closes bugs"),
            // Lintian's patterns are unanchored substrings.
            changes("lowest"),
        ];
        let fail = vec![
            changes("unknown"),
            changes("urgent"),
            changes("very high"),
            changes(""),
        ];
        ChangesTester::new(BadUrgencyInChangesFile, pass, fail).test_and_snapshot();
    }
}
