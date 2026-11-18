use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind};
use crate::lint::rule::BinaryPackageRule;

use super::declared::declared;

pub struct ConffileHasBadFileType;

declare_rule! {
    /// A `conffiles` path is installed as a directory or a symbolic link.
    ///
    /// Each Data files entry whose path equals a `conffiles` path (leading
    /// `/` removed) and that is neither a regular file nor a hardlink is
    /// reported. Directories get a trailing slash in the Location. Extra is
    /// empty. Udebs are not checked.
    ConffileHasBadFileType,
    code = "LN0777",
    tag = "conffile-has-bad-file-type",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ConffileHasBadFileType {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let declared: BTreeSet<String> =
            declared(ctx).into_iter().map(|entry| entry.path).collect();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let locations: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::Directory | EntryKind::Symlink))
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|path| declared.contains(path))
            })
            .map(location)
            .collect();
        for path in locations {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn location(entry: &Entry) -> PathBuf {
    match entry.kind() {
        EntryKind::Directory => PathBuf::from(format!("{}/", entry.path().display())),
        _ => entry.path().to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn installed(files: &[(&str, TestFile)], conffiles: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .iter()
                .map(|(path, file)| ((*path).to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
        .control_file("conffiles", TestFile::new(conffiles))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            installed(&[("etc/foo", TestFile::new("x"))], "/etc/foo\n"),
            installed(&[("etc/symlink", TestFile::symlink("foo"))], "/etc/other\n"),
            installed(&[("etc/dir", TestFile::directory())], "etc/dirx\n"),
            installed(
                &[("etc/symlink", TestFile::symlink("foo"))],
                "/etc/symlink\n",
            )
            .udeb(),
            installed(&[("etc/foo", TestFile::new("x"))], "/etc/foo\n").udeb(),
        ];
        let fail = vec![
            installed(
                &[("etc/symlink", TestFile::symlink("foo"))],
                "/etc/symlink\n",
            ),
            installed(
                &[("etc/symlink", TestFile::symlink("foo"))],
                "etc/symlink\n",
            ),
            installed(&[("etc/dir", TestFile::directory())], "/etc/dir\n"),
            installed(
                &[
                    ("etc/dir", TestFile::directory()),
                    ("etc/symlink", TestFile::symlink("foo")),
                ],
                "/etc/dir\n/etc/symlink\n",
            ),
        ];
        BinaryPackageTester::new(ConffileHasBadFileType, pass, fail).test_and_snapshot();
    }
}
