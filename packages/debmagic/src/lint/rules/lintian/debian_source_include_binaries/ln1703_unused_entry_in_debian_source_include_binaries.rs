use crate::declare_rule;
use crate::lint::context::{SourceContext, follow_symlinks, resolve_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::{Severity, SourceRule};

pub struct UnusedEntryInDebianSourceIncludeBinaries;

declare_rule! {
    /// An entry in `debian/source/include-binaries` names a path that does not
    /// exist in the patched source tree, so dpkg-source would not include
    /// anything for it.
    ///
    /// Blank lines and lines starting with `#` are skipped; the other lines are
    /// trimmed at both ends. An entry is used when it resolves through symbolic
    /// links, whatever its kind. A `debian/source` or `include-binaries` symlink
    /// is followed inside the tree, as lintian's `resolve_path` and `is_open_ok`
    /// do. Extra is the trimmed entry. The Location is the entry's line. Reported
    /// at info severity. Applies to the Source tree and Source package.
    UnusedEntryInDebianSourceIncludeBinaries,
    code = "LN1703",
    tag = "unused-entry-in-debian-source-include-binaries",
    default_selected = false,
    experimental = false,
}

impl SourceRule for UnusedEntryInDebianSourceIncludeBinaries {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(item) = include_binaries(files) else {
            return;
        };
        let Ok(bytes) = files.read(item) else {
            return;
        };
        let item_path = item.path().to_path_buf();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let mut hits = Vec::new();
        for (index, raw) in text.split('\n').enumerate() {
            let line = raw.trim();
            if raw.trim().is_empty() || raw.starts_with('#') {
                continue;
            }
            if resolve_by_path(files, line).is_none() {
                let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
                hits.push((line.to_string(), number));
            }
        }
        for (entry, number) in hits {
            ctx.diagnostic(
                Diagnostic::warn(entry)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: item_path.clone(),
                        line: Some(number),
                        column: None,
                    }),
            );
        }
    }
}

/// The `include-binaries` file, read through `debian/source` when that is a
/// symbolic link. `None` when either is missing, or the file is not open-able.
fn include_binaries(files: &FileTree) -> Option<&Entry> {
    let source_dir = resolve_symlinks(files, files.get("debian/source")?)?;
    if source_dir.kind() != EntryKind::Directory {
        return None;
    }
    let item = files.get(source_dir.path().join("include-binaries"))?;
    follow_symlinks(files, item)
}

/// The entry at `path` once symbolic links are followed (lintian's `resolve_path`).
fn resolve_by_path<'t>(files: &'t FileTree, path: &str) -> Option<&'t Entry> {
    resolve_symlinks(files, files.get(path)?)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/source", TestFile::directory()),
            ("debian/source/include-binaries", TestFile::new(body)),
            ("debian/icon.png", TestFile::new("png")),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/source/format", TestFile::new("3.0 (native)\n"))],
            tree("debian/icon.png\n"),
            tree("\n# debian/missing.png\n  debian/icon.png  \n"),
            vec![
                ("debian/source", TestFile::symlink("alt")),
                ("debian/alt", TestFile::directory()),
                (
                    "debian/alt/include-binaries",
                    TestFile::new("debian/icon.png\n"),
                ),
                ("debian/icon.png", TestFile::new("png")),
            ],
        ];
        let fail = vec![
            tree("debian/missing.png\n"),
            tree("debian/icon.png\n\ndebian/icon.gif\n  debian/icon.jpg\n"),
            vec![
                ("debian/source", TestFile::symlink("alt")),
                ("debian/alt", TestFile::directory()),
                (
                    "debian/alt/include-binaries",
                    TestFile::new("debian/gone.png\n"),
                ),
            ],
        ];
        SourceTreeTester::new(UnusedEntryInDebianSourceIncludeBinaries, pass, fail)
            .test_and_snapshot();
    }
}
