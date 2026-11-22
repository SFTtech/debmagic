use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

use super::{bpo_numbers, major_distribution};

pub struct BackportsUploadHasIncorrectVersionNumber;

declare_rule! {
    /// A backports target of the `Distribution` field (a target with
    /// `backports` in its name) whose version does not carry the backports
    /// number that release expects: the version must be `~bpoN+M`, and `N`
    /// must be 60 for squeeze, 8 for jessie, 70 for `wheezy-backports` and 7
    /// for `wheezy-backports-sloppy`. The extra is the version and the
    /// target, in that order. Reported once per backports target.
    BackportsUploadHasIncorrectVersionNumber,
    code = "LN1482",
    tag = "backports-upload-has-incorrect-version-number",
    default_selected = true,
    experimental = false,
}

impl ChangesRule for BackportsUploadHasIncorrectVersionNumber {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let changes = ctx.changes();
        let version = changes.version().unwrap_or("");
        let numbers = bpo_numbers(version);
        let targets: Vec<String> = changes
            .distributions()
            .into_iter()
            .filter(|target| *target != "UNRELEASED" && target.contains("backports"))
            .map(str::to_owned)
            .collect();
        let mut bad = Vec::new();
        for target in targets {
            let major = major_distribution(&target);
            let distnumber = numbers.map(|(distnumber, _)| distnumber);
            let incorrect = match distnumber {
                None => true,
                Some(number) => {
                    (major == "squeeze" && number != "60")
                        || (target == "wheezy-backports" && number != "70")
                        || (target == "wheezy-backports-sloppy" && number != "7")
                        || (major == "jessie" && number != "8")
                }
            };
            if incorrect {
                bad.push(format!("{version} {target}"));
            }
        }
        for extra in bad {
            ctx.diagnostic(Diagnostic::error(extra));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    fn changes(version: &str, distribution: &str) -> ChangesCase {
        ChangesCase::from(format!(
            "Source: example\nVersion: {version}\nDistribution: {distribution}\nChanges:\n example ({version}) {distribution}; urgency=low\n .\n   * change\n"
        ))
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes("1.0-1", "unstable"),
            changes("1.0~bpo12+1", "bookworm"),
            changes("1.0~bpo12+1", "bookworm-backports"),
            changes("1.0~bpo60+1", "squeeze-backports"),
            changes("1.0~bpo8+1", "jessie-backports"),
            changes("1.0~bpo70+1", "wheezy-backports"),
            changes("1.0~bpo7+1", "wheezy-backports-sloppy"),
            changes("1.0~bpo12+1", "UNRELEASED"),
            // Any other backports release takes any number.
            changes("1.0~bpo13+2", "trixie-backports bookworm"),
            changes("1.0~bpo60+1", "bookworm-backports"),
        ];
        let fail = vec![
            changes("1.0-1", "bookworm-backports"),
            changes("1.0~bpo7+1", "squeeze-backports"),
            changes("1.0~bpo8+1", "squeeze-backports"),
            changes("1.0~bpo12+1", "wheezy-backports"),
            changes("1.0~bpo8+1", "wheezy-backports-sloppy"),
            changes("1.0~bpo9+1", "jessie-backports"),
            // Two backports targets: one report each.
            changes("1.0~bpo12+1", "squeeze-backports wheezy-backports-sloppy"),
        ];
        ChangesTester::new(BackportsUploadHasIncorrectVersionNumber, pass, fail)
            .test_and_snapshot();
    }
}
