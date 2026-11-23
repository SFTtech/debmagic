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
}
