use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5_scan::copyright_texts;

pub struct FilesExcludedWithoutCopyrightFormat10;

declare_rule! {
    /// `Files-Excluded` is used without the copyright-format 1.0 Format.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` as text
    /// (symlinks and files that are not UTF-8 are skipped), whatever their
    /// format. Reported when a line starts with `Files-Excluded:` and no line
    /// is a `Format:` URI ending in `/doc/packaging-manuals/copyright-format/1.0/`.
    /// Extra is empty; the Location is the copyright file.
    FilesExcludedWithoutCopyrightFormat10,
    code = "LN0879",
    tag = "files-excluded-without-copyright-format-1.0",
    default_selected = true,
    experimental = false,
}

impl SourceRule for FilesExcludedWithoutCopyrightFormat10 {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, text) in copyright_texts(ctx) {
            if !FILES_EXCLUDED.is_match(&text) || FORMAT_1_0.is_match(&text) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

static FILES_EXCLUDED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^Files-Excluded:").expect("valid regex"));
static FORMAT_1_0: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^Format:.*/doc/packaging-manuals/copyright-format/1.0/?$")
        .expect("valid regex")
});

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const FORMAT_1_0: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn copyright(format: &str) -> TestFile {
        TestFile::new(format!(
            "{format}Files-Excluded: *.jar\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/copyright", copyright(FORMAT_1_0))],
            vec![(
                "debian/copyright",
                TestFile::new(
                    "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n",
                ),
            )],
            // Without Files-Excluded there is nothing to check.
            vec![(
                "debian/copyright",
                TestFile::new(
                    "Format: https://example.org/\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n",
                ),
            )],
        ];
        let fail = vec![
            vec![(
                "debian/copyright",
                copyright("Format: https://example.org/\n"),
            )],
            vec![("debian/copyright", copyright(""))],
            vec![(
                "debian/copyright",
                TestFile::new(
                    "Format-Specification: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\nFiles-Excluded: *.jar\n",
                ),
            )],
        ];
        SourceTreeTester::new(FilesExcludedWithoutCopyrightFormat10, pass, fail)
            .test_and_snapshot();
    }
}
