use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;
use super::ln1325_quilt_patch_missing_description::readable;

pub struct QuiltPatchUsesDpatchPlaceholder;

declare_rule! {
    /// A quilt patch still uses a dpatch placeholder line.
    ///
    /// Runs when Format is `3.x (quilt)` or Build-Depends-All satisfies
    /// `quilt`, and `debian/patches/series` is readable. Each patch the series
    /// names that is a file is scanned up to the first line starting with
    /// `---`. A line that is the dpatch shebang `#!/bin/sh /usr/share/dpatch/dpatch-run`
    /// or `@DPATCH@` (leading and trailing whitespace allowed) gets a
    /// Diagnostic at that line, with the patch as Location. Info, not selected
    /// by default. Patches that are not files are the other Tag's business.
    QuiltPatchUsesDpatchPlaceholder,
    code = "LN1529",
    tag = "quilt-patch-uses-dpatch-placeholder",
    default_selected = false,
    experimental = false,
}

static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:#!\s*/bin/sh\s+/usr/share/dpatch/dpatch-run|\s*@DPATCH@)\s*$")
        .expect("valid regex")
});

impl SourceRule for QuiltPatchUsesDpatchPlaceholder {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let format = source_format(ctx);
        let quilt_format = is_quilt_format(&format);
        let uses_quilt = quilt_format
            || ctx.debian_control().is_some_and(|control| {
                control
                    .source()
                    .relations_and(BUILD_DEPENDS_ALL)
                    .satisfies("quilt")
            });
        if !uses_quilt {
            return;
        }
        let Some(series) = ctx
            .quilt_series()
            .into_iter()
            .find(|(path, _)| path == &PathBuf::from("debian/patches/series"))
            .map(|(_, series)| series)
        else {
            return;
        };
        let names: Vec<String> = series
            .patches()
            .iter()
            .map(|patch| patch.name().to_string())
            .collect();
        let Some(files) = ctx.files() else {
            return;
        };
        let mut hits: Vec<(PathBuf, u32)> = Vec::new();
        for name in names {
            let path = PathBuf::from("debian/patches").join(&name);
            let Some(bytes) = readable(files, &path) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            for (index, line) in text.split('\n').enumerate() {
                if line.starts_with("---") {
                    break;
                }
                if PLACEHOLDER.is_match(line) {
                    hits.push((path.clone(), index as u32 + 1));
                }
            }
        }
        for (path, line) in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: Some(line),
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

    const SERIES: &str = "01-fix.patch\n";

    fn quilt(patch: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/source/format", TestFile::new("3.0 (quilt)\n")),
            ("debian/patches/series", TestFile::new(SERIES)),
            ("debian/patches/01-fix.patch", TestFile::new(patch)),
        ]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            quilt("Description: fix\n---\n--- a/foo\n+++ b/foo\n"),
            // Only lines before the first `---` count.
            quilt("Description: fix\n---\n#!/bin/sh /usr/share/dpatch/dpatch-run\n@DPATCH@\n"),
            // Other shebangs and text that merely contains the placeholder do not match.
            quilt("Description: fix\n#!/bin/sh /usr/share/other\n@DPATCH@ now\n---\n"),
            // Not a quilt source: the Check does not run.
            vec![
                ("debian/source/format", TestFile::new("1.0\n")),
                ("debian/patches/series", TestFile::new(SERIES)),
                (
                    "debian/patches/01-fix.patch",
                    TestFile::new("#!/bin/sh /usr/share/dpatch/dpatch-run\n---\n"),
                ),
            ],
            // A series that names no readable file yields nothing.
            vec![
                ("debian/source/format", TestFile::new("3.0 (quilt)\n")),
                ("debian/patches/series", TestFile::new("missing.patch\n")),
            ],
        ];
        let fail = vec![
            quilt("#!/bin/sh /usr/share/dpatch/dpatch-run\nDescription: fix\n---\n"),
            quilt("Description: fix\n\n   @DPATCH@  \n---\n"),
            quilt("Description: fix\n#!/bin/sh   /usr/share/dpatch/dpatch-run\n@DPATCH@\n---\n"),
        ];
        SourceTreeTester::new(QuiltPatchUsesDpatchPlaceholder, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(quilt("Description: fix\n---\n"))];
        let fail = vec![SourcePackageCase::from(quilt("@DPATCH@\n---\n"))];
        SourcePackageTester::new(QuiltPatchUsesDpatchPlaceholder, pass, fail).test_and_snapshot();
    }
}
