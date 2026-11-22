use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::installed::short_lines;

pub struct DebianChangelogLineTooShort;

declare_rule! {
    /// A change line of the newest installed changelog entry is a bullet
    /// followed by at most five characters of text, such as `* dh 11`. A text
    /// ending in `:` is exempt. Extra is the text after the bullet. The line
    /// number is counted as for `debian-changelog-line-too-long`. Pedantic and
    /// not selected by default, as in lintian. Udebs are not checked.
    DebianChangelogLineTooShort,
    code = "LN1309",
    tag = "debian-changelog-line-too-short",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for DebianChangelogLineTooShort {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = ctx.installed_changelog() else {
            return;
        };
        let Some(latest) = installed.parsed.entries().first() else {
            return;
        };
        let path = installed.path.clone();
        for (line, excerpt) in short_lines(&installed.text, latest.changes()) {
            ctx.diagnostic(
                Diagnostic::warn(excerpt)
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
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

    const LONG_ENOUGH: &str = "\
example (1.0-1) unstable; urgency=low

  * This change line is long enough to be fine.

 -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000
";

    /// Three short bullets (`dh 11`, `R³`, `12345`) and one ending in a colon.
    const SHORT: &str = "\
example (1.0-1) unstable; urgency=low

  * dh 11
  * R³
  * 12345
  * Fixes:

 -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000
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
                gzip(LONG_ENOUGH),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/changelog.Debian.gz".to_string(),
                gzip(SHORT),
            )])
            .udeb(),
        ];
        let fail = vec![BinaryPackageCase::from(vec![(
            "usr/share/doc/example/changelog.Debian.gz".to_string(),
            gzip(SHORT),
        )])];
        BinaryPackageTester::new(DebianChangelogLineTooShort, pass, fail).test_and_snapshot();
    }
}
