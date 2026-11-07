use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::{installed_changelog, long_lines};

pub struct DebianChangelogLineTooLong;

declare_rule! {
    /// A change line of the newest installed changelog entry is 82 characters
    /// or more.
    ///
    /// The count includes the extra leading space the changelog parser stores.
    /// A bullet (or `see`) followed by a single token is exempt. The line
    /// number is counted from the start of the file through the first indented
    /// line, then through the change lines. Extra is empty. Udebs are not
    /// checked.
    DebianChangelogLineTooLong,
    code = "LN0311",
    tag = "debian-changelog-line-too-long",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebianChangelogLineTooLong {
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
        let path = installed.path.clone();
        for line in long_lines(&installed.text, latest.changes()) {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: path.clone(),
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const SHORT: &str = "\
example (1.0-1) unstable; urgency=low

  * Short line.

 -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000
";

    /// The long prose lines are the recipe's lines 8 and 15; the URL lines are
    /// exempt.
    const LONG: &str = "\
changelog-file-general (1:1.0-1) unstable; urgency=low

  * Test: changelog-file-general
  * Upload to experimental.
  * improbable-bug-number-in-closes (Closes: #1234)
  * Another bug closer without a colon (Closes #555555)
  * This line is too long since it contains words and words and words but doesn't
    have any wrapping except at column 81.
  * http://www.example.com/but-a-really-long-url-does-not-count-as-a-long-line-at-all
    See http://www.example.com/but-a-really-long-url-does-not-count-as-a-long-line-at-all
    even if preceded by See.
  * Misspelled Closes (Close: #666666)

  * Here is another really long in the changelog, and it would be really nice if a
    certain program would report its line number correctly.  Especially since,
    we have an extra empty line in the start of the entry.

 -- A Uthor <a@example.com>  Tue, 31 Mar 2026 17:46:22 +0200
";

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/changelog.Debian.gz".to_string(),
                gzip(SHORT),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/changelog.Debian.gz".to_string(),
                gzip(LONG),
            )])
            .udeb(),
        ];
        let fail = vec![BinaryPackageCase::from(vec![(
            "usr/share/doc/example/changelog.Debian.gz".to_string(),
            gzip(LONG),
        )])];
        BinaryPackageTester::new(DebianChangelogLineTooLong, pass, fail).test_and_snapshot();
    }
}
