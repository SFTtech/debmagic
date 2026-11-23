use crate::declare_rule;
use crate::lint::debian_changelog::DebianChangelog;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::group_context::GroupContext;
use crate::lint::rule::{GroupRule, Severity};

pub struct UnreleasedChangelogDistribution;

declare_rule! {
    /// The latest entry of the source's `debian/changelog` names the
    /// distribution `UNRELEASED`, and the Group's `.changes` is a signed
    /// file. The hint has no extra and is located at the heading of the
    /// latest entry (`debian/changelog:1` in practice). Severity is info and
    /// the Tag is not selected by default, as in lintian.
    ///
    /// A Group Rule: it needs the `.changes` and the source's Patched files,
    /// so it runs on a Group with a `.dsc` and a `.changes`. The signature
    /// test is lintian's `BEGIN PGP SIGNATURE.*END PGP SIGNATURE` over the
    /// raw `.changes`, and the distribution compare is exact and
    /// case-sensitive. An empty changelog, a changelog that is not UTF-8 or
    /// is missing from the Patched files, or an unsigned `.changes` gives
    /// nothing.
    UnreleasedChangelogDistribution,
    code = "LN1664",
    tag = "unreleased-changelog-distribution",
    default_selected = false,
    experimental = false,
}

impl GroupRule for UnreleasedChangelogDistribution {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let Some(changes) = ctx.changes() else {
            return;
        };
        if !changes.is_signed() {
            return;
        }
        let Some(tree) = ctx.patched_files() else {
            return;
        };
        let Some(entry) = tree.get("debian/changelog") else {
            return;
        };
        let Ok(bytes) = tree.read(entry) else {
            return;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            return;
        };
        let changelog = DebianChangelog::parse(text);
        let Some(latest) = changelog.entries().first() else {
            return;
        };
        if latest.distribution() != "UNRELEASED" {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn("")
                .with_severity(Severity::Info)
                .with_location(Location {
                    path: "debian/changelog".into(),
                    line: Some(latest.line()),
                    column: None,
                }),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester, TestCase};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";
    const SIGNED: &str = "-----BEGIN PGP SIGNED MESSAGE-----\nHash: SHA256\n\nSource: example\nVersion: 1.0\nDistribution: unstable\n-----BEGIN PGP SIGNATURE-----\n\nfake\n-----END PGP SIGNATURE-----\n";
    const UNSIGNED: &str = "Source: example\nVersion: 1.0\nDistribution: unstable\n";

    fn changelog(latest: &str) -> String {
        format!(
            "example (1.0) {latest}; urgency=low\n\n  * Release.\n\n -- A B <a@example.org>  Tue, 31 Mar 2026 17:46:22 +0200\n"
        )
    }

    fn with_changes(changes: &str, latest: &str) -> GroupCase {
        GroupCase::new(DSC)
            .changes(changes)
            .source_files(TestCase::from(vec![(
                "debian/changelog",
                changelog(latest).as_str(),
            )]))
    }

    #[test]
    fn group_rule() {
        let pass = vec![
            // No `.changes`.
            GroupCase::new(DSC).source_files(TestCase::from(vec![(
                "debian/changelog",
                changelog("UNRELEASED").as_str(),
            )])),
            // An unsigned `.changes`.
            with_changes(UNSIGNED, "UNRELEASED"),
            // Only the exact distribution counts.
            with_changes(SIGNED, "unreleased"),
            with_changes(SIGNED, "unstable"),
            with_changes(SIGNED, "UNRELEASED experimental"),
            // Only the latest entry counts.
            GroupCase::new(DSC)
                .changes(SIGNED)
                .source_files(TestCase::from(vec![(
                    "debian/changelog",
                    format!("{}\n{}", changelog("unstable"), changelog("UNRELEASED")).as_str(),
                )])),
            // No changelog in the Patched files.
            GroupCase::new(DSC).changes(SIGNED),
        ];
        let fail = vec![with_changes(SIGNED, "UNRELEASED")];
        GroupTester::new(UnreleasedChangelogDistribution, pass, fail).test_and_snapshot();
    }
}
