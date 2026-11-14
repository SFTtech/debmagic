use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::{installed_changelog, installed_news};

pub struct DebianNewsEntryHasUnknownVersion;

declare_rule! {
    /// The newest `NEWS.Debian` entry names a version the changelog never had.
    ///
    /// Needs an installed Debian changelog with at least one entry and
    /// `usr/share/doc/<pkg>/NEWS.Debian.gz` (that capitalization), gunzipped
    /// when the resolved name ends in `.gz`. The newest news entry has a
    /// version that is not, literally, the version of any changelog entry.
    /// Extra is that version. The heading line of the news entry is the
    /// Location. Udebs are not checked.
    DebianNewsEntryHasUnknownVersion,
    code = "LN0665",
    tag = "debian-news-entry-has-unknown-version",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebianNewsEntryHasUnknownVersion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(changelog) = installed_changelog(ctx) else {
            return;
        };
        if changelog.parsed.entries().is_empty() {
            return;
        }
        let Some(news) = installed_news(ctx) else {
            return;
        };
        let Some(latest) = news.parsed.entries().first() else {
            return;
        };
        let version = latest.version();
        if version.is_empty()
            || changelog
                .parsed
                .entries()
                .iter()
                .any(|entry| entry.version() == version)
        {
            return;
        }
        let version = version.to_string();
        let line = latest.line();
        ctx.diagnostic(Diagnostic::warn(version).with_location(Location {
            path: news.path,
            line: Some(line),
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * A change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn files(changelog: &str, news: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![
            (
                "usr/share/doc/example/changelog.Debian.gz".to_string(),
                gzip(changelog),
            ),
            (
                "usr/share/doc/example/NEWS.Debian.gz".to_string(),
                gzip(news),
            ),
        ])
    }

    #[test]
    fn test() {
        let history = format!("{}{}", entry("1.0-2"), entry("1.0-1"));
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            files(&history, &entry("1.0-1")),
            files(&history, &entry("1.0-2")),
            // The version must match literally, epoch included.
            files(&history, &format!("{}{}", entry("1.0-1"), entry("9.9-9"))),
            // No changelog, or no news.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/NEWS.Debian.gz".to_string(),
                gzip(&entry("2.0")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/changelog.Debian.gz".to_string(),
                gzip(&history),
            )]),
            files(&history, &entry("2.0")).udeb(),
        ];
        let fail = vec![
            files(&history, &entry("2.0")),
            files(&history, &entry("1:1.0-1")),
            files(&history, &format!("{}{}", entry("2.0"), entry("1.0-1"))),
        ];
        BinaryPackageTester::new(DebianNewsEntryHasUnknownVersion, pass, fail).test_and_snapshot();
    }
}
