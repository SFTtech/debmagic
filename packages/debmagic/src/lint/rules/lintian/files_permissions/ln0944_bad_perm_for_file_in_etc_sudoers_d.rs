use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct BadPermForFileInEtcSudoersD;

declare_rule! {
    /// A file in `etc/sudoers.d/` whose mode is not `0440`.
    ///
    /// Walks Data files regular files and hardlinks under `etc/sudoers.d/`
    /// that are neither executable nor setuid or setgid. Reported as an error
    /// with the extra `<mode> != 0440`, because sudo refuses sudoers files
    /// with other modes. The path is the Location. Udebs are checked too.
    BadPermForFileInEtcSudoersD,
    code = "LN0944",
    tag = "bad-perm-for-file-in-etc-sudoers.d",
    default_selected = true,
    experimental = false,
}

const SUDOERS_FILE: u32 = 0o440;
const SETUID_OR_SETGID: u32 = 0o6000;

impl BinaryPackageRule for BadPermForFileInEtcSudoersD {
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
                    && entry
                        .path()
                        .as_os_str()
                        .as_bytes()
                        .starts_with(b"etc/sudoers.d/")
                    && entry.mode() != SUDOERS_FILE
            })
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::error(format!("{mode:04o} != {SUDOERS_FILE:04o}")).with_location(
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

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    fn file(mode: u32) -> TestFile {
        TestFile::new("Defaults env_reset\n").mode(mode)
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            tree(vec![("etc/sudoers.d/sudotest", file(0o440))]),
            // Outside etc/sudoers.d: other Tags.
            tree(vec![("etc/sudoers", file(0o644))]),
            tree(vec![("etc/sudoers.d-other/x", file(0o644))]),
            // Executable: other Tags.
            tree(vec![("etc/sudoers.d/exec", file(0o755))]),
            // Setuid or setgid: other Tags.
            tree(vec![("etc/sudoers.d/suid", file(0o4644))]),
            // Directories are not files.
            tree(vec![(
                "etc/sudoers.d/dir",
                TestFile::directory().mode(0o755),
            )]),
        ];
        let fail = vec![
            tree(vec![("etc/sudoers.d/sudotest", file(0o644))]),
            tree(vec![("etc/sudoers.d/world", file(0o604))]),
            tree(vec![("etc/sudoers.d/owner-rw", file(0o600))]),
            tree(vec![("etc/sudoers.d/hard", file(0o644).hardlink())]),
            tree(vec![("etc/sudoers.d/x", file(0o640))]).udeb(),
        ];
        BinaryPackageTester::new(BadPermForFileInEtcSudoersD, pass, fail).test_and_snapshot();
    }
}
