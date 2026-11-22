use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

pub struct UploadHasBackportsVersionNumber;

declare_rule! {
    /// A `.changes` whose `Version` has the backports form `~bpoN+M` (with an
    /// optional `+salsaci` suffix) names `sid`, `unstable` or `experimental`
    /// in its `Distribution`. A backports version is for a backports suite.
    /// The extra is the version and the distribution: `1.0~bpo1+1 unstable`,
    /// one hint per such distribution, in the order of the field.
    ///
    /// `UNRELEASED` is not a target, as in lintian's Check. The distributions
    /// are the words of the field, compared as written.
    UploadHasBackportsVersionNumber,
    code = "LN1497",
    tag = "upload-has-backports-version-number",
    default_selected = true,
    experimental = false,
}

/// lintian's backports version pattern, anchored at the end of the version.
fn backports_version() -> &'static Regex {
    static PATTERN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"~bpo\d+\+\d+(\+salsaci(\+\d+)*)?$").expect("backports version regex")
    });
    &PATTERN
}

impl ChangesRule for UploadHasBackportsVersionNumber {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let changes = ctx.changes();
        let version = changes.version().unwrap_or("");
        if !backports_version().is_match(version) {
            return;
        }
        let mut messages = Vec::new();
        for distribution in changes.distributions() {
            if distribution != "UNRELEASED"
                && matches!(distribution, "sid" | "unstable" | "experimental")
            {
                messages.push(format!("{version} {distribution}"));
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

    fn changes(version: &str, distribution: &str) -> ChangesCase {
        ChangesCase::from(format!(
            "Source: example\nVersion: {version}\nDistribution: {distribution}\n"
        ))
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes("1.0~bpo1+1", "bookworm-backports"),
            changes("1.0~bpo1+1", "stable-backports"),
            // Not a backports form: no `+M`, or text after it.
            changes("1.0~bpo1", "unstable"),
            changes("1.0~bpo1+1x", "unstable"),
            changes("1.0-1", "unstable"),
            // UNRELEASED is not a target.
            changes("1.0~bpo1+1", "UNRELEASED"),
            // No Version field.
            ChangesCase::from("Source: example\nDistribution: unstable\n"),
        ];
        let fail = vec![
            changes("1.0~bpo1+1", "unstable"),
            changes("1.0~bpo12+3+salsaci+2", "sid"),
            changes("1.0~bpo1+1", "unstable experimental"),
            // Only the sid, unstable and experimental targets count.
            changes("1.0~bpo1+1", "UNRELEASED unstable bookworm-backports"),
        ];
        ChangesTester::new(UploadHasBackportsVersionNumber, pass, fail).test_and_snapshot();
    }
}
