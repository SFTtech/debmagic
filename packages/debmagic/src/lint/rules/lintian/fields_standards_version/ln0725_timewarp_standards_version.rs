use chrono::{DateTime, Utc};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::{policy_release_epoch, standards_version};

pub struct TimewarpStandardsVersion;

declare_rule! {
    /// The Standards-Version is a Policy release dated after the newest
    /// `debian/changelog` entry.
    ///
    /// Only a known release counts, and only a Standards-Version of three or
    /// more dot-separated numbers. Compares against the first changelog entry,
    /// and skips one without a parseable date or with distribution exactly
    /// `UNRELEASED`. Extra is the two dates, `(changelog < release)`, with times
    /// when they fall on the same day. Applies to the Source tree and Source
    /// package.
    TimewarpStandardsVersion,
    code = "LN0725",
    tag = "timewarp-standards-version",
    default_selected = true,
    experimental = false,
}

fn is_number(component: &str) -> bool {
    !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit())
}

impl SourceRule for TimewarpStandardsVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(version) = standards_version(ctx) else {
            return;
        };
        // Trailing dots are dropped, as Perl's split does.
        let components: Vec<&str> = version.trim_end_matches('.').split('.').collect();
        if components.len() < 3 || !components.iter().all(|component| is_number(component)) {
            return;
        }
        let Some(release_epoch) = policy_release_epoch(&version) else {
            return;
        };
        let Some(entry) = ctx
            .debian_changelog()
            .and_then(|changelog| changelog.entries().first())
        else {
            return;
        };
        let Some(changelog_epoch) = entry.timestamp() else {
            return;
        };
        if entry.distribution() == "UNRELEASED" || release_epoch <= changelog_epoch {
            return;
        }

        let (Some(changelog_time), Some(release_time)) = (
            DateTime::<Utc>::from_timestamp(changelog_epoch, 0),
            DateTime::<Utc>::from_timestamp(release_epoch, 0),
        ) else {
            return;
        };
        let changelog_date = changelog_time.format("%Y-%m-%d").to_string();
        let release_date = release_time.format("%Y-%m-%d").to_string();
        let extra = if changelog_date == release_date {
            format!(
                "({} < {})",
                changelog_time.format("%Y-%m-%d %H:%M:%S UTC"),
                release_time.format("%Y-%m-%d %H:%M:%S UTC"),
            )
        } else {
            format!("({changelog_date} < {release_date})")
        };
        ctx.diagnostic(Diagnostic::warn(extra));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn entry(distribution: &str, date: &str) -> String {
        format!(
            "example (1.0) {distribution}; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  {date}\n"
        )
    }

    fn changelog(distribution: &str, date: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/changelog", TestFile::new(entry(distribution, date)))]
    }

    fn control(standards_version: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nStandards-Version: {standards_version}\n\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    /// A Source tree with the given Standards-Version and first entry.
    fn tree(
        standards_version: &str,
        distribution: &str,
        date: &str,
    ) -> Vec<(&'static str, TestFile)> {
        let mut files = control(standards_version);
        files.extend(changelog(distribution, date));
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            // No changelog to compare with.
            control("4.7.2"),
            // Changelog on or after the release.
            tree("4.7.2", "unstable", "Thu, 27 Feb 2025 07:00:00 +0000"),
            tree("4.7.2", "unstable", "Sat, 01 Mar 2025 06:00:00 +0000"),
            // Explicit fourth component: the same release.
            tree("4.7.2.0", "unstable", "Sat, 01 Mar 2025 00:00:00 +0000"),
            // Unreleased, whatever the date.
            tree("4.7.2", "UNRELEASED", "Mon, 01 Jan 2024 00:00:00 +0000"),
            // Not a known release, or not three numbers.
            tree("4.7.99", "unstable", "Mon, 01 Jan 2024 00:00:00 +0000"),
            tree("4.7", "unstable", "Mon, 01 Jan 2024 00:00:00 +0000"),
            tree("4.7.2.", "unstable", "Mon, 01 Jan 2024 00:00:00 +0000"),
            tree("4.x.2", "unstable", "Mon, 01 Jan 2024 00:00:00 +0000"),
            // A date that does not parse has no timestamp.
            tree("4.7.2", "unstable", "The, 15 Apr 2004 23:33:51 +0200"),
        ];
        let fail = vec![
            tree("4.7.2", "unstable", "Wed, 01 Jan 2025 00:00:00 +0000"),
            // Same day, earlier time: the times are shown.
            tree("4.7.2", "unstable", "Thu, 27 Feb 2025 06:00:00 +0000"),
            // Another offset, a minute before the release.
            tree("4.7.2", "unstable", "Thu, 27 Feb 2025 07:34:02 +0100"),
            tree("4.7.2.0", "unstable", "Wed, 26 Feb 2025 00:00:00 +0000"),
            // `3.9.6` is `3.9.6.0`, released 2014-09-17.
            tree("3.9.6", "unstable", "Tue, 16 Sep 2014 00:00:00 +0000"),
            // `3.6.0` is `3.6.0.0`, released 2003-07-09.
            tree("3.6.0", "unstable", "Mon, 07 Jul 2003 00:00:00 +0000"),
        ];
        SourceTreeTester::new(TimewarpStandardsVersion, pass, fail).test_and_snapshot();
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
            SourcePackageCase::from(changelog("unstable", "Sat, 01 Mar 2025 00:00:00 +0000"))
                .dsc(dsc("4.7.2")),
            // The Dsc decides: debian/control's 4.7.2 would be a timewarp.
            SourcePackageCase::from({
                let mut files = control("4.7.2");
                files.extend(changelog("unstable", "Mon, 01 Jan 2024 00:00:00 +0000"));
                files
            })
            .dsc(dsc("3.9.6")),
        ];
        let fail = vec![
            SourcePackageCase::from(changelog("unstable", "Mon, 01 Jan 2024 00:00:00 +0000"))
                .dsc(dsc("4.7.2")),
            // The Dsc decides: debian/control's 3.9.6 would not be a timewarp.
            SourcePackageCase::from({
                let mut files = control("3.9.6");
                files.extend(changelog("unstable", "Mon, 01 Jan 2024 00:00:00 +0000"));
                files
            })
            .dsc(dsc("4.7.2")),
        ];
        SourcePackageTester::new(TimewarpStandardsVersion, pass, fail).test_and_snapshot();
    }
}
