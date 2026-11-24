use regex::{Regex, escape};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct DebianChangelogFileContainsObsoleteUserEmacsSettings;

declare_rule! {
    /// The installed Debian changelog carries an obsolete `add-log-mailing-address`
    /// Emacs setting.
    ///
    /// Reads the installed Debian changelog. A line containing `Local
    /// variables:` (any case; the last occurrence on the line) fixes a prefix
    /// and a suffix, and the line itself or a later one that is that prefix,
    /// optional whitespace, `add-log-mailing-address:`, anything, and that
    /// suffix is reported. Extra is empty. The offending line is the
    /// Location. Udebs are not checked.
    DebianChangelogFileContainsObsoleteUserEmacsSettings,
    code = "LN0664",
    tag = "debian-changelog-file-contains-obsolete-user-emacs-settings",
    default_selected = true,
    experimental = false,
}

/// Lines (1-based) of `text` that carry the obsolete setting.
fn obsolete_setting_lines(text: &str) -> Vec<u32> {
    let mut hits = Vec::new();
    let mut settings: Option<(String, String)> = None;
    for (index, line) in text.split('\n').enumerate() {
        // `(.*)Local\ variables:(.*)` with `/i`: greedy, so the last occurrence.
        // ASCII lowercasing keeps byte offsets.
        let folded = line.to_ascii_lowercase();
        if let Some(start) = folded.rfind("local variables:") {
            settings = Some((
                line[..start].to_string(),
                line[start + "local variables:".len()..].to_string(),
            ));
        }
        let Some((prefix, suffix)) = &settings else {
            continue;
        };
        let pattern = format!(
            r"^{}\s*add-log-mailing-address:.*{}$",
            escape(prefix),
            escape(suffix)
        );
        if Regex::new(&pattern).is_ok_and(|pattern| pattern.is_match(line)) {
            hits.push(u32::try_from(index + 1).unwrap_or(u32::MAX));
        }
    }
    hits
}

impl BinaryPackageRule for DebianChangelogFileContainsObsoleteUserEmacsSettings {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = ctx.installed_changelog() else {
            return;
        };
        for line in obsolete_setting_lines(&installed.text) {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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

    const ENTRY: &str = "\
example (1.0-1) unstable; urgency=low

  * A change.

 -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000
";

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
            file("usr/share/doc/example/changelog.Debian.gz", gzip(ENTRY)),
            // The setting without a "Local variables:" header.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&format!(
                    "{ENTRY}\n add-log-mailing-address: \"a@example.com\"\n"
                )),
            ),
            // The header, but another variable.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&format!(
                    "{ENTRY}\nLocal variables:\nmode: debian-changelog\nEnd:\n"
                )),
            ),
            // The prefix differs.
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&format!(
                    "{ENTRY}\n;; Local variables:\n;;; add-log-mailing-address: \"a@example.com\"\n"
                )),
            ),
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&format!(
                    "{ENTRY}\nLocal variables:\nadd-log-mailing-address: \"a@example.com\"\n"
                )),
            )
            .udeb(),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&format!(
                    "{ENTRY}\nLocal variables:\nadd-log-mailing-address: \"a@example.com\"\nEnd:\n"
                )),
            ),
            file(
                "usr/share/doc/example/changelog",
                TestFile::new(format!(
                    "{ENTRY}\n;; LOCAL VARIABLES: ;;\n;;   add-log-mailing-address: x ;;\n;; add-log-mailing-address: y ;;\n"
                )),
            ),
        ];
        BinaryPackageTester::new(
            DebianChangelogFileContainsObsoleteUserEmacsSettings,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
