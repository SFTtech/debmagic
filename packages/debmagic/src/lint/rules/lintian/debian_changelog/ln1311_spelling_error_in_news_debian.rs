use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::spelling::{check_spelling, group_exceptions};

use super::installed::installed_news;

pub struct SpellingErrorInNewsDebian;

declare_rule! {
    /// The newest entry of the installed `NEWS.Debian.gz` contains a
    /// misspelling from lintian's spelling lists.
    ///
    /// Reads the installed `usr/share/doc/<package>/NEWS.Debian.gz`. The whole
    /// change text of the first entry is checked, as lintian's `check_spelling`
    /// does, with the spelling exceptions of the Binary package's group. The
    /// Location is the NEWS file with no line; extra is the word and its
    /// correction. Pedantic and not selected by default, as in lintian. Udebs
    /// are checked too.
    SpellingErrorInNewsDebian,
    code = "LN1311",
    tag = "spelling-error-in-news-debian",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for SpellingErrorInNewsDebian {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(news) = installed_news(ctx) else {
            return;
        };
        let Some(newest) = news.parsed.entries().first() else {
            return;
        };
        let exceptions = group_exceptions(ctx);
        for (found, correction) in check_spelling(newest.changes(), &exceptions, true) {
            ctx.diagnostic(
                Diagnostic::warn(format!("{found} {correction}"))
                    .with_location(Location {
                        path: news.path.clone(),
                        line: None,
                        column: None,
                    })
                    .with_severity(Severity::Pedantic),
            );
        }
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

    fn news(contents: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/doc/example/NEWS.Debian.gz".to_string(),
            contents,
        )])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            news(gzip(&entry("1.0-1", "A correct entry."))),
            // Only the newest entry is checked.
            news(gzip(&format!(
                "{}{}",
                entry("1.0-2", "Correct."),
                entry("1.0-1", "Old speling.")
            ))),
            // Not compressed: the name is not NEWS.Debian.gz.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/NEWS.Debian",
                TestFile::new(entry("1.0-1", "A speling error.")),
            )]),
        ];
        let fail = vec![
            news(gzip(&entry("1.0-1", "This has a speling error."))),
            news(gzip(&entry("1.0-1", "A usefull speling error."))).udeb(),
        ];
        BinaryPackageTester::new(SpellingErrorInNewsDebian, pass, fail).test_and_snapshot();
    }
}
