use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NfsTemporaryFileInPackage;

declare_rule! {
    /// A regular file or hardlink whose name is `.nfs` followed by at least
    /// one character, in a directory.
    ///
    /// NFS writes these for files deleted while still open. The name must be
    /// preceded by a slash, so a root-level `.nfsX` is not flagged. A directory
    /// or symbolic link is not flagged. Extra is empty; the path is the
    /// Location. Udebs are checked.
    NfsTemporaryFileInPackage,
    code = "LN0790",
    tag = "nfs-temporary-file-in-package",
    default_selected = true,
    experimental = false,
}

static NFS_TEMPORARY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/\.nfs[^/]+$").expect("valid regex"));

impl BinaryPackageRule for NfsTemporaryFileInPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|path| NFS_TEMPORARY.is_match(path))
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            data("usr/share/foo/nfs", TestFile::new("x")),
            data("usr/share/foo/.nfs", TestFile::new("x")),
            data("usr/share/foo/x.nfs0000", TestFile::new("x")),
            data("usr/share/foo/.nfsdir/file", TestFile::new("x")),
            data("usr/share/foo/.nfsdir", TestFile::directory()),
            data(".nfsroot", TestFile::new("x")),
            data("usr/share/foo/.nfs0001", TestFile::symlink("file")),
        ];
        let fail = vec![
            data("usr/share/foo/.nfs0000000012345678", TestFile::new("x")),
            data("usr/share/foo/.nfs-fake-tmpfile", TestFile::new("x")),
            data("usr/share/foo/.nfs0001", TestFile::new("x").hardlink()),
            data("usr/share/foo/.nfs-fake-tmpfile", TestFile::new("x")).udeb(),
        ];
        BinaryPackageTester::new(NfsTemporaryFileInPackage, pass, fail).test_and_snapshot();
    }
}
