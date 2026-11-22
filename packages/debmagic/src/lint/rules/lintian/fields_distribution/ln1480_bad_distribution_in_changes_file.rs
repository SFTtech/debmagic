use crate::declare_rule;
use crate::lint::data;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

use super::major_distribution;

pub struct BadDistributionInChangesFile;

declare_rule! {
    /// Reports each target of the `Distribution` field of the `.changes` that
    /// is not a known distribution, once per target in field order. A target
    /// is known when its codename, with a common extension (`-backports`,
    /// `-security`, `-proposed-updates`, ...) stripped, is in lintian's
    /// `changes-file/known-dists`. `UNRELEASED` is never a target. The extra
    /// is the target as written.
    ///
    /// The list is vendored from lintian 2.144.0 with the distro-info series
    /// (`data/changes-file/known-dists`).
    BadDistributionInChangesFile,
    code = "LN1480",
    tag = "bad-distribution-in-changes-file",
    default_selected = true,
    experimental = false,
}

const KNOWN_DISTS: &str = include_str!("../../../data/changes-file/known-dists");

impl ChangesRule for BadDistributionInChangesFile {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let unknown: Vec<String> = ctx
            .changes()
            .distributions()
            .into_iter()
            .filter(|target| *target != "UNRELEASED")
            .filter(|target| {
                !data::lines(KNOWN_DISTS).any(|dist| dist == major_distribution(target))
            })
            .map(str::to_owned)
            .collect();
        for target in unknown {
            ctx.diagnostic(Diagnostic::error(target));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    fn changes(distribution: &str) -> ChangesCase {
        ChangesCase::from(format!(
            "Source: example\nVersion: 1.0-1\nDistribution: {distribution}\nChanges:\n example (1.0-1) {distribution}; urgency=low\n"
        ))
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes("unstable"),
            changes("sid experimental"),
            changes("bookworm-backports"),
            changes("bookworm-security bookworm-proposed-updates"),
            changes("trixie-lts stable-updates"),
            // A stray UNRELEASED is not a target.
            changes("UNRELEASED"),
            changes("UNRELEASED unstable"),
            ChangesCase::from("Source: example\nVersion: 1.0-1\nChanges:\n example\n"),
        ];
        let fail = vec![
            changes("wispy"),
            changes("wispy-backports"),
            changes("unstable wispy"),
            // The extension is stripped once, from the end only.
            changes("unstable-security-wispy"),
        ];
        ChangesTester::new(BadDistributionInChangesFile, pass, fail).test_and_snapshot();
    }
}
