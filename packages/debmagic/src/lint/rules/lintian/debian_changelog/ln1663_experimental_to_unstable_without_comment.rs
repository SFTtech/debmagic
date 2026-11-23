use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct ExperimentalToUnstableWithoutComment;

declare_rule! {
    /// The installed changelog's latest entry moves the package from
    /// `experimental` to `unstable` and does not say so.
    ///
    /// Needs at least two installed changelog entries. Both distributions are
    /// compared lowercased. The latest entry's changes must not contain `to
    /// unstable` or `to sid`, optionally quoted (straight or typographic
    /// quotes), anywhere, case-insensitively. A package whose
    /// `usr/share/doc/<package>` is a symlink is skipped. Extra is empty. The
    /// Location is the heading of the latest entry in the installed changelog.
    /// Severity is pedantic and the Tag is not selected by default, as in
    /// lintian. Only Binary packages are checked; the Source package's own
    /// changelog is not.
    ExperimentalToUnstableWithoutComment,
    code = "LN1663",
    tag = "experimental-to-unstable-without-comment",
    default_selected = false,
    experimental = false,
}

static TO_UNSTABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("(?i)\\bto\\s+['\"\u{2018}\u{201c}]?(?:unstable|sid)['\"\u{2019}\u{201d}]?\\b")
        .expect("valid regex")
});

impl BinaryPackageRule for ExperimentalToUnstableWithoutComment {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let doc = PathBuf::from(format!("usr/share/doc/{}", ctx.package_name()));
        if ctx
            .data_files()
            .and_then(|files| files.get(&doc))
            .is_some_and(|entry| entry.is_symlink())
        {
            return;
        }
        let Some(installed) = ctx.installed_changelog() else {
            return;
        };
        let [latest, previous, ..] = installed.parsed.entries() else {
            return;
        };
        let moved = latest.distribution().to_lowercase() == "unstable"
            && previous.distribution().to_lowercase() == "experimental";
        if !moved || TO_UNSTABLE.is_match(latest.changes()) {
            return;
        }
        let location = Location {
            path: installed.path.clone(),
            line: Some(latest.line()),
            column: None,
        };
        ctx.diagnostic(
            Diagnostic::warn("")
                .with_severity(Severity::Pedantic)
                .with_location(location),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn entry(version: &str, distribution: &str, body: &str) -> String {
        format!(
            "example ({version}) {distribution}; urgency=low\n\n  * {body}\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn changelog(text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/doc/example/changelog.Debian.gz".to_string(),
            gzip(text),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            changelog(&entry("1.0-1", "unstable", "Initial.")),
            changelog(&format!(
                "{}{}",
                entry("1.0-2", "unstable", "Moved to unstable."),
                entry("1.0-1", "experimental", "First.")
            )),
            changelog(&format!(
                "{}{}",
                entry("1.0-2", "unstable", "Ready, to 'unstable' now."),
                entry("1.0-1", "experimental", "First.")
            )),
            changelog(&format!(
                "{}{}",
                entry("1.0-2", "unstable", "Ready TO SID."),
                entry("1.0-1", "experimental", "First.")
            )),
            changelog(&format!(
                "{}{}",
                entry("1.0-2", "unstable", "Fix."),
                entry("1.0-1", "unstable", "First.")
            )),
        ];
        let fail = vec![changelog(&format!(
            "{}{}",
            entry("1.0-2", "unstable", "Fix."),
            entry("1.0-1", "experimental", "First.")
        ))];
        BinaryPackageTester::new(ExperimentalToUnstableWithoutComment, pass, fail)
            .test_and_snapshot();
    }
}
