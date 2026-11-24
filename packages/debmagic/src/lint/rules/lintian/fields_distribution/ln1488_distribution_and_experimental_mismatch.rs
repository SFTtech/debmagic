use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

use super::distribution_mismatch;

pub struct DistributionAndExperimentalMismatch;

declare_rule! {
    /// The first changelog entry of the `.changes` names `experimental`, and
    /// the `Distribution` field does not. Reported once, with no extra, in
    /// place of `distribution-and-changes-mismatch`, which lintian does not
    /// report for the same mismatch. `sid` reads as `unstable` on both sides.
    DistributionAndExperimentalMismatch,
    code = "LN1488",
    tag = "distribution-and-experimental-mismatch",
    default_selected = true,
    experimental = false,
}

impl ChangesRule for DistributionAndExperimentalMismatch {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let Some(mismatch) = distribution_mismatch(ctx.changes()) else {
            return;
        };
        if mismatch
            .from_changes
            .iter()
            .any(|word| word == "experimental")
        {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    fn changes(distribution: &str, changelog: &str) -> ChangesCase {
        ChangesCase::from(format!(
            "Source: example\nVersion: 1.0-1\nDistribution: {distribution}\nChanges:\n example (1.0-1) {changelog}; urgency=low\n .\n   * change\n"
        ))
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes("unstable", "unstable"),
            changes("experimental", "experimental"),
            changes("unstable experimental", "experimental unstable"),
            // Experimental only in Distribution is not this mismatch.
            changes("experimental", "unstable"),
            changes("experimental", "UNRELEASED"),
        ];
        let fail = vec![
            changes("unstable", "experimental"),
            changes("stable", "unstable experimental"),
            changes("sid", "experimental"),
            changes("UNRELEASED", "experimental"),
        ];
        ChangesTester::new(DistributionAndExperimentalMismatch, pass, fail).test_and_snapshot();
    }
}
