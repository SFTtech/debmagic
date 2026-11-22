use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

use super::distribution_mismatch;

pub struct DistributionAndChangesMismatch;

declare_rule! {
    /// The `Distribution` field of the `.changes` and the distributions of the
    /// first changelog entry in its `Changes` field name different sets. The
    /// extra is the words only `Distribution` names, then the words only the
    /// changelog names, in their order. `sid` reads as `unstable` on both
    /// sides. Not reported when the changelog names `experimental` only
    /// beyond `Distribution` (that is `distribution-and-experimental-mismatch`),
    /// when the changelog has no distribution, or when `UNRELEASED` changes
    /// are not named by `Distribution`.
    ///
    /// Lintian reports the words of each side with `List::Compare`; this Rule
    /// keeps every occurrence, so a repeated word is reported as often as it
    /// occurs.
    DistributionAndChangesMismatch,
    code = "LN1487",
    tag = "distribution-and-changes-mismatch",
    default_selected = true,
    experimental = false,
}

impl ChangesRule for DistributionAndChangesMismatch {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let Some(mismatch) = distribution_mismatch(ctx.changes()) else {
            return;
        };
        if mismatch
            .from_changes
            .iter()
            .any(|word| word == "experimental")
        {
            return;
        }
        let extra: Vec<String> = mismatch
            .from_distribution
            .into_iter()
            .chain(mismatch.from_changes)
            .collect();
        ctx.diagnostic(Diagnostic::warn(extra.join(" ")));
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
            changes("sid", "unstable"),
            changes("unstable", "sid"),
            changes("unstable experimental", "unstable experimental"),
            // Only the first entry is compared, and a changelog without a
            // distribution is not.
            ChangesCase::from(
                "Source: example\nDistribution: stable\nChanges:\n example (1.0-1) stable\n",
            ),
            ChangesCase::from(
                "Source: example\nDistribution: stable\nChanges:\n example (1.0-1); urgency=low\n",
            ),
            // Changes naming UNRELEASED that Distribution does not name: the
            // unreleased-changes Check returns before this comparison.
            changes("stable", "UNRELEASED"),
            // Experimental in the changelog only is another Tag.
            changes("unstable", "unstable experimental"),
            // UNRELEASED on both sides.
            changes("UNRELEASED", "UNRELEASED"),
        ];
        let fail = vec![
            changes("stable", "unstable"),
            changes("unstable", "stable"),
            changes("stable unstable", "unstable"),
            changes("bookworm", "trixie"),
        ];
        ChangesTester::new(DistributionAndChangesMismatch, pass, fail).test_and_snapshot();
    }
}
