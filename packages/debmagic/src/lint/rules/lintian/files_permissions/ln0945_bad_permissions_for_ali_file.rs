use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct BadPermissionsForAliFile;

declare_rule! {
    /// A GNAT `.ali` file under `usr/lib/` whose mode is not `0444`.
    ///
    /// Walks Data files regular files and hardlinks named `usr/lib/**.ali`
    /// that are not executable and not setuid or setgid. The GNAT compiler
    /// wants read-only library information, so any other mode is reported
    /// with no extra; the path is the Location. Udebs are checked too.
    BadPermissionsForAliFile,
    code = "LN0945",
    tag = "bad-permissions-for-ali-file",
    default_selected = true,
    experimental = false,
}

const WORLD_READABLE: u32 = 0o444;
const SETUID_OR_SETGID: u32 = 0o6000;

impl BinaryPackageRule for BadPermissionsForAliFile {
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
                    && is_ali_under_usr_lib(entry.path().as_os_str().as_bytes())
                    && entry.mode() != WORLD_READABLE
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(Diagnostic::warn(String::new()).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Lintian's `^ usr/lib/ .* [.]ali $`.
fn is_ali_under_usr_lib(path: &[u8]) -> bool {
    path.starts_with(b"usr/lib/") && path.ends_with(b".ali")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    fn file(mode: u32) -> TestFile {
        TestFile::new("GNAT ALI\n").mode(mode)
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            tree(vec![("usr/lib/some-where/sample.ali", file(0o444))]),
            // Not an .ali file, or not under usr/lib: other Tags.
            tree(vec![("usr/lib/some-where/sample.a", file(0o644))]),
            tree(vec![("usr/share/sample.ali", file(0o644))]),
            // Executable or setuid: other Tags.
            tree(vec![("usr/lib/some-where/exec.ali", file(0o755))]),
            tree(vec![("usr/lib/some-where/suid.ali", file(0o4644))]),
            // Directories are not files.
            tree(vec![(
                "usr/lib/some.ali",
                TestFile::directory().mode(0o755),
            )]),
        ];
        let fail = vec![
            tree(vec![("usr/lib/some-where/sample.ali", file(0o644))]),
            tree(vec![("usr/lib/some-where/writable.ali", file(0o664))]),
            tree(vec![("usr/lib/some-where/owner.ali", file(0o600))]),
            tree(vec![(
                "usr/lib/some-where/hard.ali",
                file(0o644).hardlink(),
            )]),
            tree(vec![("usr/lib/x.ali", file(0o640))]).udeb(),
        ];
        BinaryPackageTester::new(BadPermissionsForAliFile, pass, fail).test_and_snapshot();
    }
}
