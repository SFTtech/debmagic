use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct GitPatchesNotExported;

declare_rule! {
    /// `debian/source/git-patches` names patches that were not exported.
    ///
    /// Reads the Source tree, or the Source package Patched files. Fires
    /// when `debian/source/git-patches` is a regular file with a line that
    /// is neither blank nor a comment, and `debian/patches/series` is either
    /// not a regular file, or does not start with a comment naming
    /// `quilt-patches-deb-export-hook` followed by at least one line that is
    /// neither blank nor a comment. Location is `debian/patches/series`, or
    /// `debian/source/git-patches` when the series file is missing.
    GitPatchesNotExported,
    code = "LN0760",
    tag = "git-patches-not-exported",
    default_selected = true,
    experimental = false,
}

const EXPORT_HOOK: &[u8] = b"quilt-patches-deb-export-hook";

impl SourceRule for GitPatchesNotExported {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = files.get("debian/source/git-patches") else {
            return;
        };
        if entry.kind() != EntryKind::RegularFile {
            return;
        }
        let Ok(bytes) = files.read(entry) else {
            return;
        };
        if !lines(&bytes).any(|line| !is_comment_or_blank(line)) {
            return;
        }
        let series = files
            .get("debian/patches/series")
            .filter(|entry| entry.kind() == EntryKind::RegularFile)
            .and_then(|entry| files.read(entry).ok());
        let (exported, path) = match series {
            Some(series) => (series_exported(&series), "debian/patches/series"),
            None => (false, "debian/source/git-patches"),
        };
        if !exported {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: PathBuf::from(path),
                line: None,
                column: None,
            }));
        }
    }
}

fn lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes.split_inclusive(|&byte| byte == b'\n')
}

fn trim_start(line: &[u8]) -> &[u8] {
    let count = line
        .iter()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();
    &line[count..]
}

fn is_comment_or_blank(line: &[u8]) -> bool {
    let trimmed = trim_start(line);
    trimmed.first().is_none_or(|&byte| byte == b'#')
}

/// The first line is the export hook comment, and some later line is an entry.
fn series_exported(series: &[u8]) -> bool {
    let mut lines = lines(series);
    let first = lines.next().unwrap_or_default();
    let trimmed = trim_start(first);
    let hook = trimmed.first() == Some(&b'#')
        && trimmed[1..]
            .windows(EXPORT_HOOK.len())
            .any(|window| window == EXPORT_HOOK);
    hook && lines.any(|line| !is_comment_or_blank(line))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const HOOK: &str = "# quilt-patches-deb-export-hook\n";

    fn gitpatches(content: &str) -> (&'static str, TestFile) {
        ("debian/source/git-patches", TestFile::new(content))
    }

    fn series(content: &str) -> (&'static str, TestFile) {
        ("debian/patches/series", TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            vec![gitpatches("")],
            vec![gitpatches("# only a comment\n\n   \n")],
            vec![
                gitpatches("upstream/1.0..patches/1.0\n"),
                series(&format!("{HOOK}fix.patch\n")),
            ],
            // A patch entry is needed.
            vec![gitpatches("   # comment\n"), series("fix.patch\n")],
            vec![("debian/source/git-patches", TestFile::directory())],
        ];
        let fail = vec![
            vec![gitpatches("upstream/1.0..patches/1.0\n")],
            vec![gitpatches("upstream/1.0..patches/1.0\n"), series("")],
            // Blank and comment lines do not count as entries.
            vec![
                gitpatches("upstream/1.0..patches/1.0\n"),
                series(&format!("{HOOK}\n  # x\n")),
            ],
            vec![
                gitpatches("upstream/1.0..patches/1.0\n"),
                series(&format!("{HOOK}# x\n")),
            ],
            // The hook must be on the first line.
            vec![
                gitpatches("upstream/1.0..patches/1.0\n"),
                series(&format!("# some\n{HOOK}fix.patch\n")),
            ],
            // A comment without the hook.
            vec![
                gitpatches("upstream/1.0..patches/1.0\n"),
                series("# generated\nfix.patch\n"),
            ],
            vec![
                gitpatches("upstream/1.0..patches/1.0\n"),
                ("debian/patches/series", TestFile::directory()),
            ],
        ];
        SourceTreeTester::new(GitPatchesNotExported, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![
            gitpatches("upstream/1.0..patches/1.0\n"),
            series(&format!("{HOOK}fix.patch\n")),
        ])];
        let fail = vec![SourcePackageCase::from(vec![gitpatches(
            "upstream/1.0..patches/1.0\n",
        )])];
        SourcePackageTester::new(GitPatchesNotExported, pass, fail).test_and_snapshot();
    }
}
