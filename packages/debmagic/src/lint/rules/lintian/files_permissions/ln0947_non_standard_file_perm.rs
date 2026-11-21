use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NonStandardFilePerm;

declare_rule! {
    /// A non-executable file whose mode is not `0644`.
    ///
    /// Walks Data files regular files and hardlinks that are neither
    /// executable nor setuid or setgid. Reported with the extra
    /// `<mode> != 0644`; the path is the Location. These are left to other
    /// Rules, as lintian does: `etc/sudoers.d/` files (LN0944), `usr/lib/**.ali`
    /// files that are not `0444` (LN0945), `etc/backup.d/` files at `0600`,
    /// and game data in `var/games/` or `var/lib/games/` at `0664`, which
    /// lintian accepts for the `root/games` owner. Entry has no owner yet, so
    /// that game data is assumed to be owned by `root/games`. Udebs are
    /// checked too.
    NonStandardFilePerm,
    code = "LN0947",
    tag = "non-standard-file-perm",
    default_selected = true,
    experimental = false,
}

const STANDARD_FILE: u32 = 0o644;
const WORLD_READABLE: u32 = 0o444;
const BACKUP_NINJA_FILE: u32 = 0o600;
const GAME_DATA: u32 = 0o664;
const SETUID_OR_SETGID: u32 = 0o6000;

impl BinaryPackageRule for NonStandardFilePerm {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                    && !entry.is_executable_file()
                    && entry.mode() & SETUID_OR_SETGID == 0
                    && entry.mode() != STANDARD_FILE
                    && !is_left_to_other_rules(entry.path().as_os_str().as_bytes(), entry.mode())
            })
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("{mode:04o} != {STANDARD_FILE:04o}")).with_location(
                    Location {
                        path,
                        line: None,
                        column: None,
                    },
                ),
            );
        }
    }
}

/// The paths and modes lintian's Check handles before its generic
/// `non-standard-file-perm` report, as its early returns do.
fn is_left_to_other_rules(path: &[u8], mode: u32) -> bool {
    let is_ali = path.starts_with(b"usr/lib/") && path.ends_with(b".ali");
    // A `.ali` file at `0444` falls through to the generic report in lintian.
    (is_ali && mode != WORLD_READABLE)
        || path.starts_with(b"etc/sudoers.d/")
        || (mode == BACKUP_NINJA_FILE && path.starts_with(b"etc/backup.d/"))
        || (mode == GAME_DATA
            && (path.starts_with(b"var/games/") || path.starts_with(b"var/lib/games/")))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    fn file(mode: u32) -> TestFile {
        TestFile::new("data\n").mode(mode)
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            tree(vec![("usr/share/doc/pkg/README", file(0o644))]),
            // Executable or setuid/setgid: other Tags.
            tree(vec![("usr/bin/tool", file(0o755))]),
            tree(vec![("usr/bin/suid", file(0o4644))]),
            // Left to other Rules, as lintian does.
            tree(vec![("etc/sudoers.d/sudotest", file(0o604))]),
            tree(vec![("usr/lib/some-where/sample.ali", file(0o600))]),
            tree(vec![("etc/backup.d/job", file(0o600))]),
            tree(vec![("var/games/pkg/score", file(0o664))]),
            tree(vec![("var/lib/games/pkg/score", file(0o664))]),
            // Directories are not files.
            tree(vec![(
                "usr/share/doc/pkg",
                TestFile::directory().mode(0o755),
            )]),
        ];
        let fail = vec![
            tree(vec![("usr/share/doc/binary/read-only", file(0o444))]),
            tree(vec![("usr/share/doc/binary/world-write", file(0o666))]),
            tree(vec![("usr/share/doc/binary/owner-only", file(0o600))]),
            tree(vec![("etc/backup.d/other", file(0o640))]),
            // Game data outside the owner-specific paths, or other modes there.
            tree(vec![("var/games/pkg/other", file(0o640))]),
            tree(vec![("var/lib/pkg/score", file(0o664))]),
            // An .ali file at 0444 falls through to the generic report.
            tree(vec![("usr/lib/some-where/sample.ali", file(0o444))]),
            tree(vec![("usr/share/doc/binary/hard", file(0o444).hardlink())]),
            tree(vec![("usr/share/doc/binary/udeb", file(0o444))]).udeb(),
        ];
        BinaryPackageTester::new(NonStandardFilePerm, pass, fail).test_and_snapshot();
    }
}
