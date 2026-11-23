use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep5_scan::{copyright_texts, native, repacked};

pub struct RepackagedSourceNotAdvertised;

declare_rule! {
    /// A copyright file with `Files-Excluded` in a copyright-format 1.0 file for
    /// a Source package that is not repacked.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` (symlinks
    /// and files that are not UTF-8 are skipped). A `Files-Excluded` field with
    /// a `Format` of the released copyright-format 1.0 URI says the upstream
    /// tarball was repacked, which the Source package's version does not show
    /// (lintian's `repacked`). Native packages are exempt. The Location is the
    /// copyright file, with no line and an empty extra. Info severity, not
    /// selected by default.
    RepackagedSourceNotAdvertised,
    code = "LN1649",
    tag = "repackaged-source-not-advertised",
    default_selected = false,
    experimental = false,
}

static FILES_EXCLUDED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^Files-Excluded:").expect("valid regex"));
static FORMAT_1_0: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^Format:.*/doc/packaging-manuals/copyright-format/1\.0/?$")
        .expect("valid regex")
});

impl SourceRule for RepackagedSourceNotAdvertised {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if repacked(ctx) || native(ctx) {
            return;
        }
        for (path, text) in copyright_texts(ctx) {
            if FILES_EXCLUDED.is_match(&text) && FORMAT_1_0.is_match(&text) {
                ctx.diagnostic(
                    Diagnostic::warn("")
                        .with_severity(Severity::Info)
                        .with_location(Location {
                            path,
                            line: None,
                            column: None,
                        }),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const FORMAT: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn changelog(version: &str) -> TestFile {
        TestFile::new(format!(
            "example ({version}) unstable; urgency=medium\n\n  * Release.\n\n -- Someone <someone@example.org>  Mon, 01 Jan 2024 00:00:00 +0000\n"
        ))
    }

    fn copyright(files_excluded: bool) -> TestFile {
        let excluded = if files_excluded {
            "Files-Excluded: old/*\n"
        } else {
            ""
        };
        TestFile::new(format!(
            "{FORMAT}Source: https://example.org/\n{excluded}\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            // Nothing excluded.
            vec![
                ("debian/changelog", changelog("1.0-1")),
                ("debian/copyright", copyright(false)),
            ],
            // Repacked upstream: the version says so.
            vec![
                ("debian/changelog", changelog("1.0+dfsg-1")),
                ("debian/copyright", copyright(true)),
            ],
            // A native package has no upstream tarball to repack.
            vec![
                ("debian/changelog", changelog("1.0")),
                ("debian/copyright", copyright(true)),
            ],
            // Files-Excluded with another Format is another Tag's case.
            vec![
                ("debian/changelog", changelog("1.0-1")),
                (
                    "debian/copyright",
                    TestFile::new(
                        "Format: https://example.org/\nFiles-Excluded: x\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n",
                    ),
                ),
            ],
        ];
        let fail = vec![vec![
            ("debian/changelog", changelog("1.0-1")),
            ("debian/copyright", copyright(true)),
        ]];
        SourceTreeTester::new(RepackagedSourceNotAdvertised, pass, fail).test_and_snapshot();
    }
}
