use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct BackupFileInPackage;

declare_rule! {
    /// A regular file or hardlink whose name looks like an editor backup or
    /// autosave file.
    ///
    /// Emacs backups end in `~`, Emacs autosaves are `#name#`, and Vim swap
    /// files are `.name.swp`. A `#` pair needs at least one character between
    /// the two, so `##` is not flagged. A directory or symbolic link is not
    /// flagged. Extra is empty; the path is the Location. Udebs are checked.
    BackupFileInPackage,
    code = "LN0789",
    tag = "backup-file-in-package",
    default_selected = true,
    experimental = false,
}

static BACKUP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"~$|#[^/]+#$|/\.[^/]+\.swp$").expect("valid regex"));

impl BinaryPackageRule for BackupFileInPackage {
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
                    .is_some_and(|path| BACKUP.is_match(path))
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
            data("usr/share/foo/file", TestFile::new("x")),
            data("usr/share/foo/file.bak", TestFile::new("x")),
            data("usr/share/foo/file.swp", TestFile::new("x")),
            data("usr/share/foo/file~x", TestFile::new("x")),
            data("usr/share/foo/#file", TestFile::new("x")),
            data("usr/share/foo/##", TestFile::new("x")),
            data("usr/share/foo/#dir#/file", TestFile::new("x")),
            data("usr/share/foo/backup~", TestFile::directory()),
            data("usr/share/foo/#dir#", TestFile::directory()),
            data("usr/share/foo/link~", TestFile::symlink("file")),
        ];
        let fail = vec![
            data("usr/share/foo/file~", TestFile::new("x")),
            data("usr/share/foo/~", TestFile::new("x")),
            data("usr/share/foo/#file#", TestFile::new("x")),
            data("usr/share/foo/#a#", TestFile::new("x")),
            data("usr/share/foo/.file.swp", TestFile::new("x")),
            data("usr/share/foo/file~", TestFile::new("x").hardlink()),
            data("usr/share/foo/file~", TestFile::new("x")).udeb(),
        ];
        BinaryPackageTester::new(BackupFileInPackage, pass, fail).test_and_snapshot();
    }
}
