use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::{
    is_newer_than_latest_policy, is_policy_release, is_three_part_number,
    latest_policy_version_normalized, standards_version,
};

pub struct NewerStandardsVersion;

declare_rule! {
    /// The Standards-Version is a newer Policy release than any this lintian
    /// knows.
    ///
    /// Fires for a Standards-Version of three or more dot-separated numbers
    /// that is not a known Policy release and sorts after the newest one.
    /// Skipped when the first `debian/changelog` entry's distribution contains
    /// `backports`. Extra is the Standards-Version and the newest release, as
    /// `(current is X.Y.Z)` with the fourth component dropped. Applies to the
    /// Source tree and Source package.
    NewerStandardsVersion,
    code = "LN0775",
    tag = "newer-standards-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NewerStandardsVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(version) = standards_version(ctx) else {
            return;
        };
        if !is_three_part_number(&version)
            || is_policy_release(&version)
            || !is_newer_than_latest_policy(&version)
        {
            return;
        }
        let backports = ctx
            .debian_changelog()
            .and_then(|changelog| changelog.entries().first())
            .is_some_and(|entry| entry.distribution().contains("backports"));
        if backports {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(format!(
            "{version} (current is {})",
            latest_policy_version_normalized()
        )));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn entry(distribution: &str) -> String {
        format!(
            "example (1.0) {distribution}; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n"
        )
    }

    fn tree(standards_version: &str, distribution: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: example\nStandards-Version: {standards_version}\n\n\
                     Package: example\nArchitecture: all\nDescription: example\n extra\n"
                )),
            ),
            ("debian/changelog", TestFile::new(entry(distribution))),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("4.7.4.1", "unstable"),
            tree("4.7.4", "unstable"),
            tree("4.7.2", "unstable"),
            // Not three numbers: `invalid-standards-version`.
            tree("3.8", "unstable"),
            tree("4.x.2", "unstable"),
            // Older than the newest release, and not known.
            tree("1.2.3", "unstable"),
            tree("4.7.3.1", "unstable"),
            // Backports are not reported.
            tree("5.15.0.0", "bookworm-backports"),
        ];
        let fail = vec![
            tree("5.15.0.0", "unstable"),
            // Sorts after the newest release by its fourth component.
            tree("4.7.4.2", "unstable"),
            tree("4.7.99", "unstable"),
            tree("4.7.99", "stable"),
        ];
        SourceTreeTester::new(NewerStandardsVersion, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let dsc = |standards_version: &str| {
            Dsc::parse(&format!(
                "Source: example\nStandards-Version: {standards_version}\n"
            ))
            .expect("Dsc")
        };
        let pass = vec![
            SourcePackageCase::from(vec![("debian/changelog", TestFile::new(entry("unstable")))])
                .dsc(dsc("4.7.4.1")),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![("debian/changelog", TestFile::new(entry("unstable")))])
                .dsc(dsc("5.15.0.0")),
        ];
        SourcePackageTester::new(NewerStandardsVersion, pass, fail).test_and_snapshot();
    }
}
