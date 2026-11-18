use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::installed_changelog;

pub struct ChangelogReferencesTempSecurityIdentifier;

declare_rule! {
    /// The newest installed changelog entry cites a temporary security
    /// identifier.
    ///
    /// Reads the installed Debian changelog. The first `TEMP-` followed by
    /// seven digits, a hyphen and six hex digits in the newest entry's
    /// changes is reported. Extra is that identifier. The heading line of the
    /// entry is the Location. Udebs are not checked.
    ChangelogReferencesTempSecurityIdentifier,
    code = "LN0662",
    tag = "changelog-references-temp-security-identifier",
    default_selected = true,
    experimental = false,
}

static TEMP_IDENTIFIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"TEMP-\d{7}-[0-9a-fA-F]{6}").expect("valid regex"));

impl BinaryPackageRule for ChangelogReferencesTempSecurityIdentifier {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = installed_changelog(ctx) else {
            return;
        };
        let Some(latest) = installed.parsed.entries().first() else {
            return;
        };
        let Some(found) = TEMP_IDENTIFIER.find(latest.changes()) else {
            return;
        };
        let identifier = found.as_str().to_string();
        let line = latest.line();
        ctx.diagnostic(Diagnostic::warn(identifier).with_location(Location {
            path: installed.path,
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

    fn entry(version: &str, body: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * {body}\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
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
                gzip(&entry("1.0-1", "Fix CVE-2020-1234.")),
            ),
            // Too few digits or too few hex digits.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry(
                    "1.0-1",
                    "Fix TEMP-123456-abcdef and TEMP-1234567-abcde.",
                )),
            ),
            // Only the newest entry is read.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&format!(
                    "{}{}",
                    entry("1.0-2", "Nothing."),
                    entry("1.0-1", "Fix TEMP-1234567-abcdef.")
                )),
            ),
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fix TEMP-1234567-abcdef.")),
            )
            .udeb(),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fix TEMP-1234567-abcdef.")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry(
                    "1.0",
                    "Fix TEMP-7654321-ABCDEF and TEMP-1234567-abcdef.",
                )),
            ),
        ];
        BinaryPackageTester::new(ChangelogReferencesTempSecurityIdentifier, pass, fail)
            .test_and_snapshot();
    }
}
