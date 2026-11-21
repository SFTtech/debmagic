use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind};
use crate::lint::rule::BinaryPackageRule;

use super::owner_exceptions::is_var_games_path;

pub struct NonStandardDirPerm;

declare_rule! {
    /// A directory whose mode is not `0755`.
    ///
    /// Walks Data files directories. Reported with the extra `<mode> != 0755`,
    /// the path is the Location. Lintian accepts a few directories with
    /// owner-specific modes, and this Rule stays silent for exactly these
    /// mode, owner and path triples: `2775` owned by `root/games` below
    /// `var/games/` or `var/lib/games/`, `2775` owned by `root/staff` for
    /// `var/local`, `2775` owned by `root/src` for `usr/src`, and `1777`
    /// owned by `root/root` at or below `tmp` and `var/tmp`, and for
    /// `var/lock`. An Entry with another owner is reported. Udebs are checked
    /// too.
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
                    && !has_owner_specific_mode(entry)
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

/// Lintian's directory exceptions that depend on the owner.
fn has_owner_specific_mode(entry: &Entry) -> bool {
    let path = entry.path().as_os_str().as_bytes();
    let mode = entry.mode();
    // Lintian's directory names end in `/`, so `tmp` is `tmp/` there.
    let at_or_below = |dir: &[u8]| path == dir || path.starts_with(&[dir, &b"/"[..]].concat());
    (mode == GAME_FOLDER && entry.has_identity("root/games") && is_var_games_path(path))
        || (mode == VAR_LOCK_FOLDER
            && entry.has_identity("root/root")
            && (at_or_below(b"tmp") || at_or_below(b"var/tmp") || path == b"var/lock"))
        || (mode == VAR_LOCAL_FOLDER && entry.has_identity("root/staff") && path == b"var/local")
        || (mode == USR_SRC_FOLDER && entry.has_identity("root/src") && path == b"usr/src")
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
            // Owner-specific modes with their owner.
            tree(vec![(
                "var/games/pkg",
                dir(0o2775).owned_by("root", "games"),
            )]),
            tree(vec![(
                "var/lib/games/pkg/sub",
                dir(0o2775).owned_by("root", "games"),
            )]),
            tree(vec![("var/local", dir(0o2775).owned_by("root", "staff"))]),
            tree(vec![("usr/src", dir(0o2775).owned_by("root", "src"))]),
            tree(vec![("tmp", dir(0o1777))]),
            tree(vec![("tmp/pkg", dir(0o1777))]),
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
            // The same modes owned by root/root (the fixture default) or someone else.
            tree(vec![("var/games/pkg", dir(0o2775))]),
            tree(vec![(
                "var/games/pkg",
                dir(0o2775).owned_by("root", "staff"),
            )]),
            tree(vec![(
                "var/games/pkg",
                dir(0o2775).owned_by("alice", "games"),
            )]),
            tree(vec![("var/local", dir(0o2775))]),
            tree(vec![("var/local", dir(0o2775).owned_by("root", "games"))]),
            tree(vec![("usr/src", dir(0o2775))]),
            tree(vec![("usr/src", dir(0o2775).owned_by("root", "staff"))]),
            tree(vec![("tmp", dir(0o1777).owned_by("root", "staff"))]),
            tree(vec![("var/lock", dir(0o1777).owned_by("alice", "root"))]),
            // The same modes outside the owner-specific paths, which are
            // exact where lintian's patterns are: `var/games/` itself, and
            // a `var/local/` or `usr/src/` subdirectory.
            tree(vec![("var/games", dir(0o2775).owned_by("root", "games"))]),
            tree(vec![(
                "var/local/pkg",
                dir(0o2775).owned_by("root", "staff"),
            )]),
            tree(vec![("usr/src/pkg", dir(0o2775).owned_by("root", "src"))]),
            tree(vec![("var/lock/pkg", dir(0o1777))]),
            tree(vec![("usr/share/games", dir(0o2775))]),
            tree(vec![("var/lib/pkg", dir(0o1777))]),
            tree(vec![("usr/bin", dir(0o2755))]),
            tree(vec![("usr/share/doc/binary", dir(0o744))]).udeb(),
        ];
        BinaryPackageTester::new(NonStandardDirPerm, pass, fail).test_and_snapshot();
    }
}
