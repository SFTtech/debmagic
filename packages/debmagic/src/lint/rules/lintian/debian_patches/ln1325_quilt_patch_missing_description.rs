use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::{Severity, SourceRule};

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;

pub struct QuiltPatchMissingDescription;

declare_rule! {
    /// A patch listed in `debian/patches/series` has no description before its
    /// diff.
    ///
    /// Runs when Format is `3.x (quilt)` or Build-Depends-All satisfies
    /// `quilt`, and `debian/patches/series` is readable. Each patch the series
    /// names that is a file is read up to the first line starting with `---`.
    /// Blank lines and lead-in lines (`Index: `, `====`, `diff ...`, `index `,
    /// `From: `) are not description. A patch with no other line gets this
    /// Diagnostic, with the patch as Location. Info, not selected by default.
    /// Patches that are not files are the other Tag's business, and the
    /// spelling and template checks of the Check are not ported.
    QuiltPatchMissingDescription,
    code = "LN1325",
    tag = "quilt-patch-missing-description",
    default_selected = false,
    experimental = false,
}

static LEAD_IN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:Index: |=+$|diff .+|index |From: )").expect("valid regex"));

impl SourceRule for QuiltPatchMissingDescription {
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
        let missing: Vec<PathBuf> = names
            .iter()
            .filter_map(|name| {
                let path = PathBuf::from("debian/patches").join(name);
                let bytes = readable(files, &path)?;
                (!has_description(&bytes)).then_some(path)
            })
            .collect();
        for path in missing {
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

/// The bytes of a patch the series names as a file, following symlinks that
/// stay in the tree. `None` when it is not a file.
pub(super) fn readable(files: &FileTree, path: &PathBuf) -> Option<Vec<u8>> {
    let entry = files.get(path)?;
    let target = follow_symlinks(files, entry)?;
    if !matches!(
        target.kind(),
        crate::lint::file_tree::EntryKind::RegularFile
            | crate::lint::file_tree::EntryKind::Hardlink
    ) {
        return None;
    }
    files.read(target).ok()
}

/// Whether any line before the first `---` line is description.
fn has_description(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    for line in text.split('\n') {
        if line.starts_with("---") {
            break;
        }
        if line.trim().is_empty() || LEAD_IN.is_match(line) {
            continue;
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const SERIES: &str = "01-fix.patch\n";

    fn quilt(patch: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/source/format", TestFile::new("3.0 (quilt)\n")),
            (
                "debian/control",
                TestFile::new(
                    "Source: example\nMaintainer: Example <ex@example.com>\nStandards-Version: 4.7.2\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
                ),
            ),
            ("debian/patches/series", TestFile::new(SERIES)),
            ("debian/patches/01-fix.patch", TestFile::new(patch)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            quilt("Fix the thing.\n\n--- a/x\n+++ b/x\n"),
            quilt("Index: example\n====\nFix it.\n--- a/x\n"),
            quilt("Subject: fix\n--- a/x\n"),
        ];
        let fail = vec![
            quilt("--- a/x\n+++ b/x\n"),
            quilt("\nIndex: example\n=====\ndiff -u a b\n\n--- a/x\n+++ b/x\nFix later\n"),
        ];
        SourceTreeTester::new(QuiltPatchMissingDescription, pass, fail).test_and_snapshot();
    }
}
