use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::installed_news;

pub struct DebianNewsEntryHasStrangeDistribution;

declare_rule! {
    /// The newest `NEWS.Debian` entry is distributed as `UNRELEASED`.
    ///
    /// Reads `usr/share/doc/<pkg>/NEWS.Debian.gz` (that capitalization),
    /// gunzipped when the resolved name ends in `.gz`. Only the exact
    /// distribution word `UNRELEASED` matches. Extra is that word. The heading
    /// line is the Location. Udebs are not checked.
    DebianNewsEntryHasStrangeDistribution,
    code = "LN0364",
    tag = "debian-news-entry-has-strange-distribution",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebianNewsEntryHasStrangeDistribution {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = installed_news(ctx) else {
            return;
        };
        let Some(latest) = installed.parsed.entries().first() else {
            return;
        };
        if latest.distribution() != "UNRELEASED" {
            return;
        }
        let line = latest.line();
        ctx.diagnostic(Diagnostic::warn("UNRELEASED").with_location(Location {
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

    fn news(distribution: &str) -> String {
        format!(
            "example (1.0) {distribution}; urgency=low\n\n  * A change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn packaged(name: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(name.to_string(), file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            packaged(
                "usr/share/doc/example/NEWS.Debian.gz",
                gzip(&news("unstable")),
            ),
            packaged(
                "usr/share/doc/example/NEWS.Debian.gz",
                gzip(&news("unreleased")),
            ),
            packaged(
                "usr/share/doc/example/NEWS.Debian.gz",
                gzip(&news("UNRELEASED unstable")),
            ),
            packaged(
                "usr/share/doc/example/NEWS.Debian",
                TestFile::new(news("UNRELEASED")),
            ),
            packaged(
                "usr/share/doc/example/NEWS.Debian.gz",
                gzip(&news("UNRELEASED")),
            )
            .udeb(),
        ];
        let fail = vec![packaged(
            "usr/share/doc/example/NEWS.Debian.gz",
            gzip(&news("UNRELEASED")),
        )];
        BinaryPackageTester::new(DebianNewsEntryHasStrangeDistribution, pass, fail)
            .test_and_snapshot();
    }
}
