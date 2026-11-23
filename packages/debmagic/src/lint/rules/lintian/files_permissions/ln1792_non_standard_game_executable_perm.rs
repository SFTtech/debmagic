use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::owner_exceptions::is_game_executable_with_own_tag;

pub struct NonStandardGameExecutablePerm;

declare_rule! {
    /// An executable owned by `root/games` is not mode `2755`.
    ///
    /// Walks Data files regular files and hardlinks with any execute bit whose
    /// owner and group are `root/games`. It is reported when it is not setgid,
    /// or lacks a bit of `0755`, with the extra `<mode> != 2755`. The path is
    /// the Location. A setgid one with every `0755` bit is not reported, its
    /// other bits notwithstanding. Other owners and groups, non-executable
    /// files and directories get nothing here. Udebs are checked too.
    NonStandardGameExecutablePerm,
    code = "LN1792",
    tag = "non-standard-game-executable-perm",
    default_selected = true,
    experimental = false,
}

/// The mode lintian expects for a game executable: setgid `games`, `0755`.
const GAME_EXECUTABLE: u32 = 0o2755;

impl BinaryPackageRule for NonStandardGameExecutablePerm {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| is_game_executable_with_own_tag(entry))
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("{mode:04o} != {GAME_EXECUTABLE:04o}")).with_location(
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
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn game(mode: u32) -> TestFile {
        TestFile::new("#!/bin/sh\n")
            .mode(mode)
            .owned_by("root", "games")
    }

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    #[test]
    fn binary() {
        let pass = vec![
            tree(vec![]),
            // Setgid and `0755`: the expected mode.
            tree(vec![("usr/games/tool", game(0o2755))]),
            // Setuid as well: the elevated-privileges Tags, not this one.
            tree(vec![("usr/games/tool", game(0o6755))]),
            // Not executable.
            tree(vec![("usr/games/data", game(0o644))]),
            tree(vec![("usr/games/data", game(0o664))]),
            // Other owners and groups.
            tree(vec![(
                "usr/games/tool",
                TestFile::new("#!/bin/sh\n")
                    .mode(0o750)
                    .owned_by("root", "staff"),
            )]),
            tree(vec![(
                "usr/games/tool",
                TestFile::new("#!/bin/sh\n")
                    .mode(0o750)
                    .owned_by("alice", "games"),
            )]),
            // Directories and symlinks are not files.
            tree(vec![(
                "usr/games/dir",
                TestFile::directory().mode(0o750).owned_by("root", "games"),
            )]),
            tree(vec![(
                "usr/games/link",
                TestFile::symlink("tool")
                    .mode(0o777)
                    .owned_by("root", "games"),
            )]),
            // Not the case: the game Tag needs the `root/games` identity.
            tree(vec![(
                "usr/bin/tool",
                TestFile::new("#!/bin/sh\n").mode(0o750),
            )]),
        ];
        let fail = vec![
            // Not setgid.
            tree(vec![("usr/games/tool", game(0o755))]),
            tree(vec![("usr/games/tool", game(0o775))]),
            // Setgid but lacking a bit of `0755`.
            tree(vec![("usr/games/tool", game(0o2750))]),
            tree(vec![("usr/games/tool", game(0o750))]),
            // Hardlinks are files to lintian.
            tree(vec![("usr/games/tool", game(0o750).hardlink())]),
            // Udebs are checked too.
            tree(vec![("usr/games/tool", game(0o750))]).udeb(),
            // Non-UTF-8 name.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/games/tool\xff".to_vec()),
                game(0o750),
            )]),
        ];
        BinaryPackageTester::new(NonStandardGameExecutablePerm, pass, fail).test_and_snapshot();
    }
}
