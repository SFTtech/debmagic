use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::names::in_path_directory;

pub struct ZeroByteExecutableInPath;

declare_rule! {
    /// A regular file directly in a PATH directory is empty and executable.
    ///
    /// PATH directories are `bin/`, `sbin/`, `usr/bin/`, `usr/sbin/`, and
    /// `usr/games/`. The file must have any execute bit set (lintian's
    /// `is_executable`) and no content. Symlinks, hardlinks and directories
    /// are not flagged. Udebs are checked too. Extra is empty; the path is
    /// the Location.
    ZeroByteExecutableInPath,
    code = "LN0726",
    tag = "zero-byte-executable-in-path",
    default_selected = true,
    experimental = false,
}

/// Any of the owner, group, or other execute bits.
const EXECUTE_BITS: u32 = 0o111;

impl BinaryPackageRule for ZeroByteExecutableInPath {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile])
            .into_iter()
            .filter(|entry| in_path_directory(entry.path()))
            .filter(|entry| entry.mode() & EXECUTE_BITS != 0)
            .filter(|entry| files.read(entry).is_ok_and(|bytes| bytes.is_empty()))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn empty_executable(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("").mode(0o755)))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Empty but not executable.
            BinaryPackageCase::from(vec![
                ("usr/bin/plain", TestFile::new("").mode(0o644)),
                ("usr/sbin/readonly", TestFile::new("").mode(0o400)),
            ]),
            // Executable but not empty.
            BinaryPackageCase::from(vec![
                ("usr/bin/tool", TestFile::new("#!/bin/sh\n").mode(0o755)),
                ("bin/one-byte", TestFile::new("x").mode(0o755)),
            ]),
            // Empty executables outside PATH directories, or in a subdirectory of one.
            empty_executable(&[
                "usr/share/foo/tool",
                "usr/lib/foo/tool",
                "usr/libexec/foo/tool",
                "usr/local/bin/tool",
                "usr/bin/sub/tool",
                "usr/games/sub/tool",
                "etc/bin/tool",
            ]),
            // Directories and symlinks are not regular files.
            BinaryPackageCase::from(vec![
                ("usr/bin/dir", TestFile::directory()),
                ("usr/bin/link", TestFile::symlink("tool")),
            ]),
        ];
        let fail = vec![
            empty_executable(&[
                "bin/empty",
                "sbin/empty",
                "usr/bin/empty",
                "usr/sbin/empty",
                "usr/games/empty",
            ]),
            // Any execute bit counts, not only the owner's.
            BinaryPackageCase::from(vec![
                ("usr/bin/owner-only", TestFile::new("").mode(0o700)),
                ("usr/bin/group-only", TestFile::new("").mode(0o010)),
                ("usr/bin/other-only", TestFile::new("").mode(0o001)),
            ]),
            // A name that is not UTF-8 is still judged by its bytes.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/bin/bokm\xe5l".to_vec()),
                TestFile::new("").mode(0o755),
            )]),
            empty_executable(&["usr/bin/empty"]).udeb(),
        ];
        BinaryPackageTester::new(ZeroByteExecutableInPath, pass, fail).test_and_snapshot();
    }
}
