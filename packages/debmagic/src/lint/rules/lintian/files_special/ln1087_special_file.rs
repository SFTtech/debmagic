use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct SpecialFile;

declare_rule! {
    /// The Binary package installs a special file: a named pipe, a character
    /// device or a block device. Policy 10.6 forbids them; a program that needs
    /// a device node should create it from the `postinst` with `makedev`.
    ///
    /// Walks Data files whose kind is neither a regular file or hardlink, nor
    /// a directory, nor a symlink; only fifos and device nodes are hits. The
    /// extra is the permission bits as four octal digits (`0644`); the Location
    /// is the entry path.
    SpecialFile,
    code = "LN1087",
    tag = "special-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SpecialFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, u32)> = files
            .query()
            .kinds([
                EntryKind::Fifo,
                EntryKind::CharDevice,
                EntryKind::BlockDevice,
            ])
            .into_iter()
            .map(|entry| (entry.path().to_path_buf(), entry.mode() & 0o7777))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::error(format!("{mode:04o}")).with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("usr/bin/test", TestFile::new("x").mode(0o755))]),
            BinaryPackageCase::from(vec![("usr/lib/foo", TestFile::directory())]),
            BinaryPackageCase::from(vec![("usr/lib/foo", TestFile::symlink("/usr/share/foo"))]),
            BinaryPackageCase::from(vec![("usr/bin/test", TestFile::new("x").hardlink())]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("run/foo.fifo", TestFile::fifo().mode(0o644))]),
            BinaryPackageCase::from(vec![("dev/foo", TestFile::char_device().mode(0o600))]),
            BinaryPackageCase::from(vec![("dev/bar", TestFile::block_device().mode(0o660))]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("dev/bar", TestFile::block_device().mode(0o660))]).udeb(),
        ];
        BinaryPackageTester::new(SpecialFile, pass, fail).test_and_snapshot();
    }
}
