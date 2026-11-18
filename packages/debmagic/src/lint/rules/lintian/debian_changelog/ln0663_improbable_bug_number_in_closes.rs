use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::installed_changelog;

pub struct ImprobableBugNumberInCloses;

declare_rule! {
    /// A bug number the newest installed changelog entry closes cannot exist
    /// in the Debian bug tracker.
    ///
    /// Reads the installed Debian changelog. The bug numbers are every digit
    /// run in the `closes:` lists of the newest entry's changes (lintian's
    /// `find_closes`), in ascending order, duplicates kept. A number below
    /// 50004 (before that the tracker deleted bugs) or from 1500000 up is
    /// reported. Extra is the number as written. The heading line of the
    /// entry is the Location. Udebs are not checked.
    ImprobableBugNumberInCloses,
    code = "LN0663",
    tag = "improbable-bug-number-in-closes",
    default_selected = true,
    experimental = false,
}

const FIRST_ARCHIVED_BUG_NUMBER: u128 = 50_004;
const OUT_OF_REACH_BUG_NUMBER: u128 = 1_500_000;

static CLOSES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)closes:\s*(?:bug)?\#?\s?\d+(?:,\s*(?:bug)?\#?\s?\d+)*").expect("valid regex")
});
static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").expect("valid regex"));

/// The bug numbers of `Closes:` statements, ascending by value.
fn find_closes(changes: &str) -> Vec<&str> {
    let mut bugs: Vec<&str> = CLOSES
        .find_iter(changes)
        .flat_map(|statement| DIGITS.find_iter(statement.as_str()).map(|bug| bug.as_str()))
        .collect();
    // An unparseable (huge) number is beyond any bug number.
    bugs.sort_by_key(|bug| bug.parse::<u128>().unwrap_or(u128::MAX));
    bugs
}

impl BinaryPackageRule for ImprobableBugNumberInCloses {
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
        let line = latest.line();
        let improbable: Vec<String> = find_closes(latest.changes())
            .into_iter()
            .filter(|bug| {
                let number = bug.parse::<u128>().unwrap_or(u128::MAX);
                !(FIRST_ARCHIVED_BUG_NUMBER..OUT_OF_REACH_BUG_NUMBER).contains(&number)
            })
            .map(str::to_string)
            .collect();
        for bug in improbable {
            ctx.diagnostic(Diagnostic::warn(bug).with_location(Location {
                path: installed.path.clone(),
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
                gzip(&entry("1.0-1", "Fix. (Closes: #50004)")),
            ),
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fix. (Closes: #1499999)")),
            ),
            // Not a closes statement.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fix issue #1234.")),
            ),
            // Only the newest entry is read.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&format!(
                    "{}{}",
                    entry("1.0-2", "Nothing."),
                    entry("1.0-1", "Fix. (Closes: #12)")
                )),
            ),
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fix. (Closes: #12)")),
            )
            .udeb(),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1", "Fix. (Closes: #1234)")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry(
                    "1.0",
                    "Fix. (Closes: #1500000, #77777, #50003, bug#50003)",
                )),
            ),
        ];
        BinaryPackageTester::new(ImprobableBugNumberInCloses, pass, fail).test_and_snapshot();
    }
}
