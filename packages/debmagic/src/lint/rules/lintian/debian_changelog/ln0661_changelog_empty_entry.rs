use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ChangelogEmptyEntry;

declare_rule! {
    /// The newest installed changelog entry has no change text.
    ///
    /// Reads the installed Debian changelog (see `no-changelog` for the file
    /// names tried). The newest entry's changes, with every non-word
    /// character removed, are empty and its distribution is not exactly
    /// `UNRELEASED`. Extra is empty. The heading line of that entry is the
    /// Location. Udebs are not checked.
    ChangelogEmptyEntry,
    code = "LN0661",
    tag = "changelog-empty-entry",
    default_selected = true,
    experimental = false,
}

static NON_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\W").expect("valid regex"));

impl BinaryPackageRule for ChangelogEmptyEntry {
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
        if !NON_WORD.replace_all(latest.changes(), "").is_empty()
            || latest.distribution() == "UNRELEASED"
        {
            return;
        }
        let line = latest.line();
        ctx.diagnostic(Diagnostic::error("").with_location(Location {
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

    fn entry(distribution: &str, body: &str) -> String {
        format!(
            "example (1.0-1) {distribution}; urgency=low\n\n{body}\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
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
                gzip(&entry("unstable", "  * A change.\n")),
            ),
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("UNRELEASED", "  * .\n")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("unstable", "  * 1\n")),
            ),
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("unstable", "  * .\n")),
            )
            .udeb(),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("unstable", "  * .\n")),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(entry("unreleased", "  * -\n")),
            ),
        ];
        BinaryPackageTester::new(ChangelogEmptyEntry, pass, fail).test_and_snapshot();
    }
}
