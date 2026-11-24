use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::group_context::GroupContext;
use crate::lint::installed_changelog::read_installed_changelog;
use crate::lint::rule::GroupRule;

pub struct ChangelogDistributionDoesNotMatchChangesFile;

declare_rule! {
    /// Reports a binary package of the Group whose installed changelog names a
    /// different target distribution in its latest entry than the Group's
    /// `.changes` does in its `Distribution` field. Both are lowercased, and
    /// `unstable` and `sid` are the same distribution. The extra is the
    /// changelog's distribution, `!=` and the `.changes`'s (`experimental !=
    /// unstable`), reported on the binary package at the latest entry of its
    /// installed changelog.
    ///
    /// A Group Rule: it needs the `.changes` and the installables, so it runs
    /// on a Group with a `.dsc` and a `.changes`. Like lintian it looks at
    /// debs only (not udebs), skips a package whose `usr/share/doc/<pkg>` is a
    /// symlink, and compares only when the changelog has at least two
    /// entries (lintian's comparison sits in the block for a changelog with a
    /// previous entry). A changelog that is not UTF-8 or cannot be read is
    /// not compared.
    ChangelogDistributionDoesNotMatchChangesFile,
    code = "LN1516",
    tag = "changelog-distribution-does-not-match-changes-file",
    default_selected = true,
    experimental = false,
}

impl GroupRule for ChangelogDistributionDoesNotMatchChangesFile {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let Some(changes) = ctx.changes() else {
            return;
        };
        let changes_dist = changes
            .fields()
            .get("Distribution")
            .unwrap_or_default()
            .to_lowercase();
        for installable in ctx.installables() {
            if installable.is_udeb() {
                continue;
            }
            let Some(files) = ctx.data_files(&installable) else {
                continue;
            };
            let name = installable.name();
            let doc = PathBuf::from(format!("usr/share/doc/{name}"));
            if files.get(&doc).is_some_and(|entry| entry.is_symlink()) {
                continue;
            }
            let Some(changelog) = read_installed_changelog(files, name) else {
                continue;
            };
            let entries = changelog.parsed.entries();
            // Lintian compares in the block for a changelog with a previous entry.
            if entries.len() < 2 {
                continue;
            }
            let latest = &entries[0];
            let latest_dist = latest.distribution().to_lowercase();
            let normalize = |dist: &str| if dist == "unstable" { "sid" } else { dist }.to_string();
            if normalize(&latest_dist) == normalize(&changes_dist) {
                continue;
            }
            ctx.installable_diagnostic(
                &installable,
                Diagnostic::warn(format!("{latest_dist} != {changes_dist}")).with_location(
                    Location {
                        path: changelog.path.clone(),
                        line: Some(latest.line()),
                        column: None,
                    },
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester, TestCase};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    fn changelog(latest: &str, previous: Option<&str>) -> String {
        let mut text = format!(
            "example (1.0) {latest}; urgency=low\n\n  * Release.\n\n -- A B <a@example.org>  Tue, 31 Mar 2026 17:46:22 +0200\n"
        );
        if let Some(previous) = previous {
            text.push_str(&format!(
                "\nexample (0.9) {previous}; urgency=low\n\n  * Earlier.\n\n -- A B <a@example.org>  Mon, 01 Jan 1990 00:00:00 +0100\n"
            ));
        }
        text
    }

    fn group(distribution: &str, latest: &str, previous: Option<&str>) -> GroupCase {
        GroupCase::new(DSC)
            .changes(format!(
                "Source: example\nVersion: 1.0\nDistribution: {distribution}\n"
            ))
            .installable("example", "")
            .files(TestCase::from(vec![(
                "usr/share/doc/example/changelog",
                changelog(latest, previous).as_str(),
            )]))
    }

    #[test]
    fn group_rule() {
        let pass = vec![
            // No `.changes`.
            GroupCase::new(DSC).installable("example", ""),
            group("unstable", "unstable", Some("unstable")),
            // `unstable` and `sid` are the same, whatever the case.
            group("unstable", "sid", Some("sid")),
            group("sid", "UNSTABLE", Some("unstable")),
            // Lintian compares only with a previous entry.
            group("unstable", "experimental", None),
            // A udeb is not looked at.
            GroupCase::new(DSC)
                .changes("Source: example\nVersion: 1.0\nDistribution: unstable\n")
                .udeb("example-udeb", "")
                .files(TestCase::from(vec![(
                    "usr/share/doc/example-udeb/changelog",
                    changelog("experimental", Some("experimental")).as_str(),
                )])),
        ];
        let fail = vec![
            group("unstable", "experimental", Some("experimental")),
            group("experimental", "unstable", Some("unstable")),
            group("unstable", "bookworm", Some("sid")),
            // A multi-word distribution is one string.
            group("unstable", "unstable experimental", Some("unstable")),
        ];
        GroupTester::new(ChangelogDistributionDoesNotMatchChangesFile, pass, fail)
            .test_and_snapshot();
    }
}
