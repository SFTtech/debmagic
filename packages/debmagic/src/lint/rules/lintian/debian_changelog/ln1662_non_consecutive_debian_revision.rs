use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::source::{changelog_location, latest};
use super::version::{ParsedVersion, parse_version};

pub struct NonConsecutiveDebianRevision;

declare_rule! {
    /// The newest changelog entry's Debian revision does not follow the
    /// previous entry's.
    ///
    /// Compares the newest and the second entry, both parsed for the package's
    /// nativeness; the Tag is quiet when either does not parse. It is also quiet
    /// for a native package, and when the two entries differ in source name,
    /// epoch or upstream version, as in lintian. Reported when
    /// the Debian revision is numeric and not one more than the previous one,
    /// when a source NMU number is not one more than the previous NMU number
    /// under the same revision, or when a first source NMU is not `1`. Extra is
    /// the previous version, ` -> `, then the newest version, for example
    /// `1.0-1 -> 1.0-3`. The Location is the newest entry's heading. Severity is
    /// pedantic. The Tag is experimental and not selected by default, as in
    /// lintian.
    NonConsecutiveDebianRevision,
    code = "LN1662",
    tag = "non-consecutive-debian-revision",
    default_selected = false,
    experimental = true,
}

impl SourceRule for NonConsecutiveDebianRevision {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(latest) = latest(ctx) else {
            return;
        };
        let Some(changelog) = ctx.debian_changelog() else {
            return;
        };
        let [latest_entry, previous_entry, ..] = changelog.entries() else {
            return;
        };
        let Some(previous) = parse_version(previous_entry.version(), latest.native) else {
            return;
        };
        // Lintian compares only a non-native package's entries of one source
        // with the same epoch and upstream version; the Debian revision rules
        // apply within that line.
        if latest.native
            || latest.version.epoch != previous.epoch
            || latest.version.upstream != previous.upstream
            || latest_entry.source() != previous_entry.source()
        {
            return;
        }
        if !non_consecutive(&latest.version, &previous) {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn(format!(
                "{} -> {}",
                previous.literal, latest.version.literal
            ))
            .with_severity(Severity::Pedantic)
            .with_location(changelog_location(latest.line)),
        );
    }
}

/// Lintian's three conditions for a gap in the Debian revision or source NMU.
fn non_consecutive(latest: &ParsedVersion, previous: &ParsedVersion) -> bool {
    let number = |text: &str| text.parse::<u64>().ok();
    let numeric = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());

    let revision_gap = latest.source_nmu.is_empty()
        && numeric(&latest.maintainer_revision)
        && numeric(&previous.maintainer_revision)
        && number(&latest.maintainer_revision)
            != number(&previous.maintainer_revision).map(|n| n + 1);

    let nmu_gap = latest.maintainer_revision == previous.maintainer_revision
        && numeric(&latest.source_nmu)
        && numeric(&previous.source_nmu)
        && number(&latest.source_nmu) != number(&previous.source_nmu).map(|n| n + 1);

    let first_nmu_not_one = numeric(&latest.source_nmu)
        && previous.source_nmu.is_empty()
        && number(&latest.source_nmu) != Some(1);

    revision_gap || nmu_gap || first_nmu_not_one
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn named_entry(source: &str, version: &str) -> String {
        format!(
            "{source} ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn entry(version: &str) -> String {
        named_entry("example", version)
    }

    fn tree(changelog: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/source/format", TestFile::new("3.0 (quilt)\n")),
            ("debian/changelog", TestFile::new(changelog)),
        ]
    }

    /// Lintian compares the newest and previous entry only when epoch, upstream
    /// version and source name all match (Lintian::Check::Debian::Changelog,
    /// the `non_consecutive` block). A new upstream version starts a new sequence.
    #[test]
    fn revision_not_compared_across_upstream_change() {
        SourceTreeTester::new(
            NonConsecutiveDebianRevision,
            vec![tree(&format!("{}{}", entry("2.0-3"), entry("1.0-1")))],
            Vec::<Vec<(&str, TestFile)>>::new(),
        )
        .test();
    }

    /// Same check as above, for a changed epoch.
    #[test]
    fn revision_not_compared_across_epoch_change() {
        SourceTreeTester::new(
            NonConsecutiveDebianRevision,
            vec![tree(&format!("{}{}", entry("2:1.0-3"), entry("1.0-1")))],
            Vec::<Vec<(&str, TestFile)>>::new(),
        )
        .test();
    }

    /// Same check as above, for entries that name different source packages.
    #[test]
    fn revision_not_compared_across_source_name_change() {
        SourceTreeTester::new(
            NonConsecutiveDebianRevision,
            vec![tree(&format!(
                "{}{}",
                named_entry("other", "1.0-3"),
                entry("1.0-1")
            ))],
            Vec::<Vec<(&str, TestFile)>>::new(),
        )
        .test();
    }

    /// Lintian returns before the revision checks for a native package.
    #[test]
    fn revision_not_compared_for_native_package() {
        let native = vec![
            ("debian/source/format", TestFile::new("3.0 (native)\n")),
            (
                "debian/changelog",
                TestFile::new(format!("{}{}", entry("3"), entry("1"))),
            ),
        ];
        SourceTreeTester::new(
            NonConsecutiveDebianRevision,
            vec![native],
            Vec::<Vec<(&str, TestFile)>>::new(),
        )
        .test();
    }

    /// Boundary: with epoch and upstream equal, a revision gap is still reported.
    #[test]
    fn revision_gap_reported_when_epoch_and_upstream_match() {
        SourceTreeTester::new(
            NonConsecutiveDebianRevision,
            Vec::<Vec<(&str, TestFile)>>::new(),
            vec![
                tree(&format!("{}{}", entry("1:1.0-3"), entry("1:1.0-1"))),
                tree(&format!("{}{}", entry("1.0-3"), entry("1.0-1"))),
            ],
        )
        .test();
    }

    #[test]
    fn test() {
        let native = vec![
            ("debian/source/format", TestFile::new("3.0 (native)\n")),
            (
                "debian/changelog",
                TestFile::new(format!("{}{}", entry("3"), entry("1"))),
            ),
        ];
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree(&format!("{}{}", entry("1.0-2"), entry("1.0-1"))),
            tree(&format!("{}{}", entry("1.0-1.1"), entry("1.0-1"))),
            tree(&format!("{}{}", entry("1.0-1.2"), entry("1.0-1.1"))),
            tree(&format!("{}{}", entry("1.0-2"), entry("1.0-1.1"))),
            tree(&format!("{}{}", entry("1.0-1"), entry("1.0"))),
            tree(&format!("{}{}", entry("1.0-1"), entry("1.0-x"))),
            tree(&entry("1.0-1")),
            // A new upstream version starts its own revision sequence.
            tree(&format!("{}{}", entry("2.0-3"), entry("1.0-1"))),
            // A changed epoch starts its own revision sequence too.
            tree(&format!("{}{}", entry("2:1.0-3"), entry("1.0-1"))),
            // The entries name different source packages.
            tree(&format!(
                "{}{}",
                named_entry("other", "1.0-3"),
                entry("1.0-1")
            )),
            // A native package has no Debian revision to number.
            native,
        ];
        let fail = vec![
            tree(&format!("{}{}", entry("1.0-3"), entry("1.0-1"))),
            tree(&format!("{}{}", entry("1.0-1.3"), entry("1.0-1.1"))),
            tree(&format!("{}{}", entry("1.0-1.2"), entry("1.0-1"))),
            tree(&format!("{}{}", entry("1:1.0-3"), entry("1:1.0-1"))),
        ];
        SourceTreeTester::new(NonConsecutiveDebianRevision, pass, fail).test_and_snapshot();
    }

    /// Lintian compares the newest entry with the previous one only when both
    /// name the same source package: entries of another source are not a line.
    /// The same source with a skipped revision is still reported.
    #[test]
    fn entries_of_another_source_are_not_compared() {
        let pass = vec![tree(&format!(
            "{}{}",
            named_entry("other", "1.0-3"),
            entry("1.0-1")
        ))];
        let fail = vec![tree(&format!("{}{}", entry("1.0-3"), entry("1.0-1")))];
        SourceTreeTester::new(NonConsecutiveDebianRevision, pass, fail).test();
    }

    /// Lintian compares the revisions only within one epoch and upstream
    /// version: a changed epoch or upstream starts a new sequence. The same
    /// epoch and upstream with a skipped revision is still reported.
    #[test]
    fn entries_of_another_epoch_or_upstream_are_not_compared() {
        let pass = vec![
            tree(&format!("{}{}", entry("2:1.0-3"), entry("1.0-1"))),
            tree(&format!("{}{}", entry("2.0-3"), entry("1.0-1"))),
        ];
        let fail = vec![tree(&format!("{}{}", entry("1.0-3"), entry("1.0-1")))];
        SourceTreeTester::new(NonConsecutiveDebianRevision, pass, fail).test();
    }

    /// A native package has no Debian revision, so the Tag is quiet for it.
    #[test]
    fn native_package_is_not_compared() {
        let pass = vec![vec![
            ("debian/source/format", TestFile::new("3.0 (native)\n")),
            (
                "debian/changelog",
                TestFile::new(format!("{}{}", entry("3"), entry("1"))),
            ),
        ]];
        let fail = vec![tree(&format!("{}{}", entry("1.0-3"), entry("1.0-1")))];
        SourceTreeTester::new(NonConsecutiveDebianRevision, pass, fail).test();
    }
}
