use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NonStandardDirPerm;

declare_rule! {
    /// A directory whose mode is not `0755`.
    ///
    /// Walks Data files directories. Reported with the extra `<mode> != 0755`,
    /// the path is the Location. Lintian accepts a few directories with
    /// owner-specific modes: `2775` in `var/games/`, `var/lib/games/`,
    /// `var/local/` and `usr/src/`, and `1777` in `tmp/`, `var/tmp/` and
    /// `var/lock/`. Entry has no owner yet, so this Rule assumes the usual
    /// owner and stays silent for those mode and path pairs. Udebs are
    /// checked too.
    NonStandardDirPerm,
    code = "LN0946",
    tag = "non-standard-dir-perm",
    default_selected = true,
    experimental = false,
}

const STANDARD_FOLDER: u32 = 0o755;
const GAME_FOLDER: u32 = 0o2775;
const VAR_LOCAL_FOLDER: u32 = 0o2775;
const VAR_LOCK_FOLDER: u32 = 0o1777;
const USR_SRC_FOLDER: u32 = 0o2775;

impl BinaryPackageRule for NonStandardDirPerm {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                entry.kind() == EntryKind::Directory
                    && entry.mode() != STANDARD_FOLDER
                    && !has_owner_specific_mode(entry.mode(), entry.path().as_os_str().as_bytes())
            })
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("{mode:04o} != {STANDARD_FOLDER:04o}")).with_location(
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

/// Lintian's directory exceptions that depend on the owner, which Entry
/// does not carry; the usual owner is assumed.
fn has_owner_specific_mode(mode: u32, path: &[u8]) -> bool {
    let under = |dir: &[u8]| path == dir || path.starts_with(&[dir, &b"/"[..]].concat());
    (mode == GAME_FOLDER
        && (path.starts_with(b"var/games/") || path.starts_with(b"var/lib/games/")))
        || (mode == VAR_LOCAL_FOLDER && (path == b"var/local" || under(b"var/local")))
        || (mode == USR_SRC_FOLDER && (path == b"usr/src" || under(b"usr/src")))
        || (mode == VAR_LOCK_FOLDER && (under(b"tmp") || under(b"var/tmp") || path == b"var/lock"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    fn dir(mode: u32) -> TestFile {
        TestFile::directory().mode(mode)
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            tree(vec![("usr/share/doc/pkg", dir(0o755))]),
            // Owner-specific modes the usual owner is assumed for.
            tree(vec![("var/games/pkg", dir(0o2775))]),
            tree(vec![("var/lib/games/pkg", dir(0o2775))]),
            tree(vec![("var/local", dir(0o2775))]),
            tree(vec![("usr/src", dir(0o2775))]),
            tree(vec![("tmp", dir(0o1777))]),
            tree(vec![("var/tmp/pkg", dir(0o1777))]),
            tree(vec![("var/lock", dir(0o1777))]),
            // Files are not directories: other Tags.
            tree(vec![(
                "usr/share/doc/pkg/README",
                TestFile::new("x\n").mode(0o600),
            )]),
        ];
        let fail = vec![
            tree(vec![("usr/share/doc/binary", dir(0o744))]),
            tree(vec![("usr/share/doc/binary", dir(0o775))]),
            tree(vec![("usr/share/doc/binary", dir(0o700))]),
            // The same modes outside the owner-specific paths.
            tree(vec![("usr/share/games", dir(0o2775))]),
            tree(vec![("var/lib/pkg", dir(0o1777))]),
            tree(vec![("usr/bin", dir(0o2755))]),
            tree(vec![("usr/share/doc/binary", dir(0o744))]).udeb(),
        ];
        BinaryPackageTester::new(NonStandardDirPerm, pass, fail).test_and_snapshot();
    }
}
