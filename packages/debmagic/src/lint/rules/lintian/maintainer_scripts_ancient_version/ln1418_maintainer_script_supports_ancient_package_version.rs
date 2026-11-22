use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use chrono::DateTime;
use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_changelog::ChangelogEntry;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::maintainer_script::{Continuations, maintainer_script_lines};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct MaintainerScriptSupportsAncientPackageVersion;

declare_rule! {
    /// A maintainer script compares the package version with a version older
    /// than the oldstable release: `dpkg --compare-versions` names a version
    /// that the installed changelog has an entry for, dated before 2023-06-10
    /// (the oldstable release). Such upgrades are not supported; the check
    /// should go.
    ///
    /// The versions are the installed changelog's entries older than the
    /// release (no timestamp counts as the release, so is not listed; a later
    /// entry with the same version replaces an earlier one), without pure
    /// integers. A logical line of a maintainer script (comments removed,
    /// `\` joined) matches when it has `dpkg --compare-versions` (optionally
    /// `/usr/bin/`) at a command position, and then the version as a whole
    /// word not followed by a `.`. Extra is the version and its date in
    /// parentheses; the Subject is the script's last physical line. Severity is
    /// info and the Tag is not selected by default.
    MaintainerScriptSupportsAncientPackageVersion,
    code = "LN1418",
    tag = "maintainer-script-supports-ancient-package-version",
    default_selected = false,
    experimental = false,
}

/// dak's OLDSTABLE release, 2023-06-10 23:56:04 UTC (lintian's
/// `$OLDSTABLE_RELEASE_EPOCH`).
const OLDSTABLE_RELEASE_EPOCH: i64 = 1_686_441_364;

/// lintian's `$LEADING_PATTERN`: where a command may start.
const LEADING: &str = r#"(?:(?:^|[`&;(|{])\s*|(?:if|then|do|while|!)\s+|env(?:\s+[[:alnum:]_]+=(?:\S+|"[^"]*"|'[^']*'))*\s+)"#;

static DPKG_COMPARE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"{LEADING}(?:/usr/bin/)?dpkg\s+--compare-versions\s+"
    ))
    .expect("valid regex")
});

impl BinaryPackageRule for MaintainerScriptSupportsAncientPackageVersion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(changelog) = ctx.installed_changelog() else {
            return;
        };
        let versions = ancient_versions(changelog.parsed.entries());
        if versions.is_empty() {
            return;
        }
        let Some(files) = ctx.control_files() else {
            return;
        };
        let mut hits: Vec<(PathBuf, u32, String)> = Vec::new();
        for line in maintainer_script_lines(files, Continuations::Always) {
            for version in compared_versions(&line.text, &versions) {
                let extra = format!(
                    "{version} ({} < {})",
                    format_date(versions[version]),
                    format_date(OLDSTABLE_RELEASE_EPOCH)
                );
                hits.push((line.path.clone(), line.number, extra));
            }
        }
        for (path, line, extra) in hits {
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

/// The changelog versions older than the release, with their timestamps, as
/// lintian's `old_versions`: a later entry with the same version replaces an
/// earlier one, and an entry without a timestamp is not listed. Pure integer
/// versions are dropped where they are compared.
fn ancient_versions(entries: &[ChangelogEntry]) -> BTreeMap<String, i64> {
    let mut versions = BTreeMap::new();
    for entry in entries {
        let timestamp = entry.timestamp().unwrap_or(OLDSTABLE_RELEASE_EPOCH);
        if timestamp < OLDSTABLE_RELEASE_EPOCH {
            versions.insert(entry.version().to_string(), timestamp);
        }
    }
    versions
}

/// The old versions a logical line compares with `dpkg --compare-versions`,
/// in version order.
fn compared_versions<'v>(text: &str, versions: &'v BTreeMap<String, i64>) -> Vec<&'v str> {
    // The version may be anywhere after the command's options, so the search
    // starts at the earliest end of such a command.
    let Some(start) = DPKG_COMPARE.find_iter(text).map(|found| found.end()).min() else {
        return Vec::new();
    };
    let tail = &text[start..];
    versions
        .keys()
        .filter(|version| !version.bytes().all(|byte| byte.is_ascii_digit()))
        .filter(|version| names_version(tail, version))
        .map(String::as_str)
        .collect()
}

/// Whether `text` has `version` as a whole word that is not followed by a `.`
/// (lintian's `\b<version>(?!\.)\b`).
fn names_version(text: &str, version: &str) -> bool {
    text.match_indices(version).any(|(start, _)| {
        let end = start + version.len();
        at_word_boundary(text, start)
            && at_word_boundary(text, end)
            && !text[end..].starts_with('.')
    })
}

/// A word boundary at `index`: word characters on one side only.
fn at_word_boundary(text: &str, index: usize) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let before = text[..index].chars().next_back().is_some_and(is_word);
    let after = text[index..].chars().next().is_some_and(is_word);
    before != after
}

/// `strftime('%Y-%m-%d', gmtime $timestamp)`.
fn format_date(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|date| date.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestCase, TestFile};

    use super::*;

    /// An installed changelog whose only entry is version 0.5, from 2009.
    const CHANGELOG: &str = "example (0.5) unstable; urgency=low\n\n  * Initial release.\n\n -- Maint <m@example.org>  Sat, 07 Feb 2009 00:00:00 +0000\n";

    fn with_script(script: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/doc/example/changelog",
            TestFile::new(CHANGELOG),
        )])
        .control_file("postinst", TestFile::new(script))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(TestCase::default()),
            // No changelog, so no versions.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "postinst",
                TestFile::new("#!/bin/sh\ndpkg --compare-versions 0.5 lt 1\n"),
            ),
            with_script("#!/bin/sh\n# dpkg --compare-versions 0.5 lt 1\n"),
            with_script("#!/bin/sh\ndpkg --compare-versions 0.6 lt 1\n"),
            // Followed by a dot: a different version.
            with_script("#!/bin/sh\ndpkg --compare-versions 0.5.1 lt 1\n"),
            with_script("#!/bin/sh\necho dpkg --compare-versions 0.5 lt 1\n"),
        ];
        let fail = vec![
            with_script("#!/bin/sh\ndpkg --compare-versions 0.5 lt 1\n"),
            with_script("#!/bin/sh\nif /usr/bin/dpkg --compare-versions 0.5 lt 1; then\n  :\nfi\n"),
            with_script("#!/bin/sh\nset -e\ndpkg --compare-versions \\\n  2.0 gt 0.5\n"),
        ];
        BinaryPackageTester::new(MaintainerScriptSupportsAncientPackageVersion, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn a_version_is_a_whole_word_not_followed_by_a_dot() {
        assert!(names_version("lt 0.5 x", "0.5"));
        assert!(names_version("0.5", "0.5"));
        assert!(!names_version("0.5.1", "0.5"));
        assert!(!names_version("10.5", "0.5"));
        assert!(!names_version("0.55", "0.5"));
        // A later occurrence counts, even after one that does not.
        assert!(names_version("0.5.1 0.5 ", "0.5"));
    }

    #[test]
    fn dates_are_utc() {
        assert_eq!(format_date(0), "1970-01-01");
        assert_eq!(format_date(OLDSTABLE_RELEASE_EPOCH), "2023-06-10");
    }
}
