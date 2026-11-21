use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct ExecutableInUsrShareDocbase;

declare_rule! {
    /// A direct child of `usr/share/doc-base/` is executable.
    ///
    /// A regular file or hardlink is checked by its own mode. A symlink is
    /// followed one hop, to a name in the same directory, and the target's
    /// mode decides; the symlink's own mode is reported. Subdirectories are
    /// not children for this Rule. Extra is the permission bits in four-digit
    /// octal; the path is the Location. Udebs are checked too.
    ExecutableInUsrShareDocbase,
    code = "LN0960",
    tag = "executable-in-usr-share-docbase",
    default_selected = true,
    experimental = false,
}

const DOC_BASE: &str = "usr/share/doc-base";

impl BinaryPackageRule for ExecutableInUsrShareDocbase {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, String)> = files
            .entries()
            .iter()
            .filter(|entry| is_doc_base_child(entry.path()))
            .filter(|entry| target_is_executable(files, entry))
            .map(|entry| (entry.path().to_path_buf(), format!("{:04o}", entry.mode())))
            .collect();
        for (path, message) in hits {
            ctx.diagnostic(Diagnostic::error(message).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn is_doc_base_child(path: &Path) -> bool {
    path.parent() == Some(Path::new(DOC_BASE))
}

/// Whether the file, or the file a symlink names, is executable. A symlink
/// that does not resolve to a regular file or hardlink is not.
fn target_is_executable(files: &FileTree, entry: &Entry) -> bool {
    match entry.kind() {
        EntryKind::RegularFile | EntryKind::Hardlink => entry.is_executable_file(),
        EntryKind::Symlink => entry
            .link_target()
            .and_then(|target| files.get(Path::new(DOC_BASE).join(target)))
            .is_some_and(Entry::is_executable_file),
        EntryKind::Directory => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data(
                "usr/share/doc-base/foo.doc1",
                TestFile::new("x").mode(0o644),
            ),
            // Not a direct child.
            data(
                "usr/share/doc-base/sub/foo.doc1",
                TestFile::new("x").mode(0o755),
            ),
            data("usr/share/doc-base", TestFile::directory().mode(0o755)),
            // A symlink to a non-executable file.
            data(
                "usr/share/doc-base/link",
                TestFile::symlink("foo.doc1").mode(0o777),
            ),
        ];
        let fail = vec![
            data(
                "usr/share/doc-base/foo.doc1",
                TestFile::new("x").mode(0o755),
            ),
            data(
                "usr/share/doc-base/foo.doc1",
                TestFile::new("x").mode(0o700),
            )
            .udeb(),
            // A symlink to an executable file reports the symlink's own mode.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc-base/foo.doc1".to_string(),
                    TestFile::new("x").mode(0o755),
                ),
                (
                    "usr/share/doc-base/link".to_string(),
                    TestFile::symlink("foo.doc1").mode(0o777),
                ),
            ]),
        ];
        BinaryPackageTester::new(ExecutableInUsrShareDocbase, pass, fail).test_and_snapshot();
    }
}
