use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

use super::mailbox::complete_mailboxes;

pub struct NoPhrase;

declare_rule! {
    /// A Maintainer, Changed-By or Uploaders mailbox of the `.changes` has an
    /// address but no name (the phrase). The extra is the field name and the
    /// mailbox as lintian formats it: `Changed-By someone@example.com`.
    ///
    /// A mailbox with no user or no host is not reported here, as lintian
    /// returns before the phrase test for an incomplete address; duplicates
    /// are judged once, by their format.
    ///
    /// This Rule covers the `.changes` only. Lintian's Check reads the same
    /// fields of the source and binary packages too, which is not ported yet,
    /// so `no-phrase` is only reported for a `.changes`.
    NoPhrase,
    code = "LN1494",
    tag = "no-phrase",
    default_selected = true,
    experimental = false,
}

const ROLES: &[&str] = &["Maintainer", "Changed-By", "Uploaders"];

impl ChangesRule for NoPhrase {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let mut messages = Vec::new();
        for role in ROLES {
            for mailbox in complete_mailboxes(ctx.changes().fields(), role) {
                if mailbox.phrase.is_empty() {
                    messages.push(format!("{role} {}", mailbox.format()));
                }
            }
        }
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    fn changes(fields: &str) -> ChangesCase {
        ChangesCase::from(format!("Source: example\nVersion: 1.0\n{fields}"))
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes("Changed-By: Jane Doe <jane@example.com>\n"),
            changes("Maintainer: \"Doe, Jane\" <jane@example.com>\n"),
            changes("Maintainer: Jane <jane@example.com>, Bob <bob@example.org>\n"),
            // Incomplete addresses are not judged for the phrase.
            changes("Maintainer: nobody@\n"),
            changes("Maintainer: nobody@ <x>\n"),
            // No Maintainer, Changed-By or Uploaders.
            changes("Distribution: unstable\n"),
        ];
        let fail = vec![
            changes("Changed-By: someone@example.com\n"),
            changes("Maintainer: someone@example.com\n"),
            changes("Changed-By: Jane <jane@example.com>\nUploaders: bob@example.org\n"),
            // The same bare address twice is reported once.
            changes("Uploaders: bob@example.org, bob@example.org\n"),
        ];
        ChangesTester::new(NoPhrase, pass, fail).test_and_snapshot();
    }
}
