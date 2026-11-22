use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct NewPackageUsesDateBasedVersionNumber;

declare_rule! {
    /// The only installed changelog entry is a first upload with a date-based version.
    ///
    /// Reads the installed changelog (see `no-changelog` for the names tried).
    /// Fires when the changelog has exactly one entry, its version ends in `-1`,
    /// and the version starts with eight digits (`YYYYMMDD-1`). Extra is the
    /// version followed by `(better: 0~<version>)`; the heading line of the entry
    /// is the Location. Changelogs with more than one entry are not checked.
    /// Udebs are not checked.
    NewPackageUsesDateBasedVersionNumber,
    code = "LN0872",
    tag = "new-package-uses-date-based-version-number",
    default_selected = true,
    experimental = false,
}

static DATE_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{8}").expect("valid regex"));

impl BinaryPackageRule for NewPackageUsesDateBasedVersionNumber {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = ctx.installed_changelog() else {
            return;
        };
        let [latest] = installed.parsed.entries() else {
            return;
        };
        let version = latest.version();
        if !version.ends_with("-1") || !DATE_PREFIX.is_match(version) {
            return;
        }
        let extra = format!("{version} (better: 0~{version})");
        let line = latest.line();
        ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
            path: installed.path.clone(),
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
            "example ({version}) unstable; urgency=low\n\n  * Initial release.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn file(name: &str, contents: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(name.to_string(), contents)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("0~20180219-1")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry("1.20180219-1")),
            ),
            // Several entries: not checked.
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(format!("{}{}", entry("20180219-1"), entry("20180101-1"))),
            ),
            // Not a first upload (revision is not 1).
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("20180219-2")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry("20180219-1")),
            )
            .udeb(),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("20180219-1")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("20180219-1")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry("20180219-1")),
            ),
        ];
        BinaryPackageTester::new(NewPackageUsesDateBasedVersionNumber, pass, fail)
            .test_and_snapshot();
    }
}
