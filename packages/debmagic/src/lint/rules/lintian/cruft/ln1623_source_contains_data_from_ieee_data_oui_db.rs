use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text::{self, BadLicense};
use crate::lint::rule::{Severity, SourceRule};

pub struct SourceContainsDataFromIeeeDataOuiDb;

declare_rule! {
    /// A Patched file is a copy of the IEEE OUI database that `ieee-data`
    /// ships: its text carries the credit `Dr. B. Struck`, the author of the
    /// listing. Reported at pedantic severity with an empty extra and the path
    /// as the Location, the pointer form lintian's `cruft` Check emits.
    ///
    /// The file's cleaned text is matched as lintian's `cruft` Check does: a
    /// keyword, the sentence and the regex. Skipped in a non-free Source
    /// package (see `license_text::is_non_free`), and paths under `.pc/` are
    /// skipped.
    ///
    /// Differs from lintian: the file is matched whole, not in 16 KiB
    /// sliding-window blocks.
    SourceContainsDataFromIeeeDataOuiDb,
    code = "LN1623",
    tag = "source-contains-data-from-ieee-data-oui-db",
    default_selected = false,
    experimental = false,
}

static IEEE_DATA: LazyLock<BadLicense> = LazyLock::new(|| {
    BadLicense::new(
        &["struck", "scitex", "racore"],
        &["dr. b. struck"],
        Some(r"dr. [ ] b. [ ] struck"),
    )
});

impl SourceRule for SourceContainsDataFromIeeeDataOuiDb {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if license_text::is_non_free(ctx) {
            return;
        }
        let texts = license_text::shared_text_files(ctx);
        let flagged: Vec<_> = texts
            .iter()
            .filter(|file| IEEE_DATA.matches(file))
            .map(|file| file.path.clone())
            .collect();
        for path in flagged {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const NON_FREE_CONTROL: &str = "Source: example\n\
         Section: non-free/libs\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const OUI: &str = "# OUI database, from IEEE data.\n# Dr. B. Struck\n00:00:0C Cisco\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("src/oui.txt", TestFile::new("00:00:0C Cisco\n")),
                // The name of the author is not the sentence.
                (
                    "src/author.txt",
                    TestFile::new("Written by Dr. Struck in 2001.\n"),
                ),
            ],
            // Quilt's `.pc/` is not the packaging's file.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (".pc/oui.db", TestFile::new(OUI)),
            ],
            // A non-free package is not reported.
            vec![
                ("debian/control", TestFile::from(NON_FREE_CONTROL)),
                ("src/oui.db", TestFile::new(OUI)),
            ],
            // A hardlink is not read, as lintian's `full_text_check` does not.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("src/oui.db", TestFile::new(OUI).hardlink()),
            ],
        ];
        let fail = vec![vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("src/oui.db/oui.db", TestFile::new(OUI)),
        ]];
        SourceTreeTester::new(SourceContainsDataFromIeeeDataOuiDb, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::from(CONTROL),
        )])];
        let fail = vec![SourcePackageCase::from(vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("src/oui.db", TestFile::new(OUI)),
        ])];
        SourcePackageTester::new(SourceContainsDataFromIeeeDataOuiDb, pass, fail)
            .test_and_snapshot();
    }
}
