use crate::declare_rule;
use crate::lint::data;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::GroupContext;
use crate::lint::rule::GroupRule;

pub struct InconsistentMaintainer;

declare_rule! {
    /// Reports a Group whose `.changes` has a different `Maintainer` than its
    /// `.dsc`, when the `.changes` targets a distribution of
    /// `changes-file/known-dists` (an upload to a derivative may change the
    /// Maintainer, see the Ubuntu wiki on the `Debian Maintainer` field). The
    /// extra is the `.changes`'s Maintainer, ` (changes vs. source) ` and the
    /// `.dsc`'s.
    ///
    /// A Group Rule that reports on the `.changes`: it needs the `.dsc` of the
    /// Group too, so it does not run on a Group without both. The
    /// `Distribution` field is matched as a whole against the list, so a
    /// `.changes` for several distributions is never recognized (as in
    /// lintian). A missing `Maintainer` counts as empty. The list is vendored
    /// from lintian 2.144.0 with the distro-info series.
    InconsistentMaintainer,
    code = "LN1491",
    tag = "inconsistent-maintainer",
    default_selected = true,
    experimental = false,
}

const KNOWN_DISTS: &str = include_str!("../../data/changes-file/known-dists");

impl GroupRule for InconsistentMaintainer {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let (Some(changes), Some(dsc)) = (ctx.changes(), ctx.dsc()) else {
            return;
        };
        let changes_maintainer = changes.fields().get("Maintainer").unwrap_or_default();
        let distribution = changes.fields().get("Distribution").unwrap_or_default();
        let source_maintainer = dsc.fields().get("Maintainer").unwrap_or_default();
        if changes_maintainer == source_maintainer
            || !data::lines(KNOWN_DISTS).any(|known| known == distribution)
        {
            return;
        }
        let extra = format!("{changes_maintainer} (changes vs. source) {source_maintainer}");
        ctx.changes_diagnostic(Diagnostic::error(extra));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester};

    use super::*;

    fn group(dsc_maintainer: &str, changes_maintainer: &str, distribution: &str) -> GroupCase {
        GroupCase::new(format!(
            "Source: example\nVersion: 1.0\nMaintainer: {dsc_maintainer}\n"
        ))
        .changes(format!(
            "Source: example\nVersion: 1.0\nDistribution: {distribution}\nMaintainer: {changes_maintainer}\n"
        ))
    }

    #[test]
    fn group_rule() {
        let pass = vec![
            group("A <a@example.org>", "A <a@example.org>", "unstable"),
            // A derivative's distribution may change the Maintainer.
            group("A <a@example.org>", "B <b@example.org>", "jammy"),
            // The field is matched as a whole.
            group(
                "A <a@example.org>",
                "B <b@example.org>",
                "unstable experimental",
            ),
            // No `.changes`.
            GroupCase::new("Source: example\nVersion: 1.0\nMaintainer: A <a@example.org>\n"),
        ];
        let fail = vec![
            group("A <a@example.org>", "B <b@example.org>", "unstable"),
            group("A <a@example.org>", "B <b@example.org>", "bookworm"),
            // A `.changes` with no Maintainer against a `.dsc` that has one.
            GroupCase::new("Source: example\nVersion: 1.0\nMaintainer: A <a@example.org>\n")
                .changes("Source: example\nVersion: 1.0\nDistribution: sid\n"),
        ];
        GroupTester::new(InconsistentMaintainer, pass, fail).test_and_snapshot();
    }
}
