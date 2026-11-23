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
    /// nativeness; the Tag is quiet when either does not parse. Reported when
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
        let Some(previous_entry) = ctx
            .debian_changelog()
            .and_then(|changelog| changelog.entries().get(1))
        else {
            return;
        };
        let Some(previous) = parse_version(previous_entry.version(), latest.native) else {
            return;
        };
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

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(changelog: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/source/format", TestFile::new("3.0 (quilt)\n")),
            ("debian/changelog", TestFile::new(changelog)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree(&format!("{}{}", entry("1.0-2"), entry("1.0-1"))),
            tree(&format!("{}{}", entry("1.0-1.1"), entry("1.0-1"))),
            tree(&format!("{}{}", entry("1.0-1.2"), entry("1.0-1.1"))),
            tree(&format!("{}{}", entry("1.0-2"), entry("1.0-1.1"))),
            tree(&format!("{}{}", entry("1.0-1"), entry("1.0"))),
            tree(&format!("{}{}", entry("1.0-1"), entry("1.0-x"))),
            tree(&entry("1.0-1")),
        ];
        let fail = vec![
            tree(&format!("{}{}", entry("1.0-3"), entry("1.0-1"))),
            tree(&format!("{}{}", entry("1.0-1.3"), entry("1.0-1.1"))),
            tree(&format!("{}{}", entry("1.0-1.2"), entry("1.0-1"))),
        ];
        SourceTreeTester::new(NonConsecutiveDebianRevision, pass, fail).test_and_snapshot();
    }
}
