use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::installed_news;

pub struct SyntaxErrorInDebianNewsFile;

declare_rule! {
    /// `NEWS.Debian.gz` is not a valid Debian changelog.
    ///
    /// Reads `usr/share/doc/<pkg>/NEWS.Debian.gz` (that capitalization),
    /// gunzipped when the resolved name ends in `.gz`. Each parser error is
    /// one Diagnostic. Extra is the quoted condition. Udebs are not checked.
    SyntaxErrorInDebianNewsFile,
    code = "LN0362",
    tag = "syntax-error-in-debian-news-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SyntaxErrorInDebianNewsFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = installed_news(ctx) else {
            return;
        };
        let path = installed.path.clone();
        for error in installed.parsed.errors() {
            let line = error.line();
            let message = error.message();
            ctx.diagnostic(
                Diagnostic::warn(format!("\"{message}\"")).with_location(Location {
                    path: path.clone(),
                    line: Some(line),
                    column: None,
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const VALID: &str = "\
example (1.0) unstable; urgency=low

  * A change.

 -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000
";

    const BROKEN: &str = "\
changelog-file-news-general (1.0) UNRELEASED; urgency=high

  * This is a major change.

 -- Russ Allbery <rra@debian.org>  Tue, 31 Mar 2026 17:46:22 +0200
changelog-file-news-general (0.9) unstable; urgency=low

  Another entry, this one with a syntax error in the date.

 -- Russ Allbery <rra@debian.org>  Tue, 31 Mar 2026 17:46:22 +0200 0
";

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
            packaged("usr/share/doc/example/NEWS.Debian.gz", gzip(VALID)),
            packaged("usr/share/doc/example/NEWS.Debian", TestFile::new(BROKEN)),
            packaged("usr/share/doc/example/news.debian.gz", gzip(BROKEN)),
            packaged("usr/share/doc/example/NEWS.Debian.gz", gzip(BROKEN)).udeb(),
        ];
        let fail = vec![packaged(
            "usr/share/doc/example/NEWS.Debian.gz",
            gzip(BROKEN),
        )];
        BinaryPackageTester::new(SyntaxErrorInDebianNewsFile, pass, fail).test_and_snapshot();
    }
}
