use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::owner_exceptions::is_game_executable_with_own_tag;

pub struct NonStandardSetuidExecutablePerm;

declare_rule! {
    /// A setuid or setgid file whose other permission bits are not `0755`.
    ///
    /// Walks Data files regular files and hardlinks with the setuid or setgid
    /// bit. Reported with the octal mode as the extra, for example `4744` or
    /// `4555`, unless the mode without the setuid and setgid bits is `0755`,
    /// or the mode is exactly `4754` (lintian's historic setgid exemption).
    /// The path is the Location. Udebs are checked too. A `root/games`
    /// executable that is not setgid, or lacks a bit of `0755`, is skipped:
    /// lintian reports it as `non-standard-game-executable-perm` instead (no
    /// Rule for that Tag here, so it gets no hint).
    NonStandardSetuidExecutablePerm,
    code = "LN0943",
    tag = "non-standard-setuid-executable-perm",
    default_selected = true,
    experimental = false,
}

const SETUID_OR_SETGID: u32 = 0o6000;
const STANDARD_EXECUTABLE: u32 = 0o755;
const SETGID_EXECUTABLE: u32 = 0o4754;

impl BinaryPackageRule for NonStandardSetuidExecutablePerm {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                    && entry.mode() & SETUID_OR_SETGID != 0
                    && (entry.mode() & !SETUID_OR_SETGID != STANDARD_EXECUTABLE)
                    && entry.mode() != SETGID_EXECUTABLE
                    && !is_game_executable_with_own_tag(entry)
            })
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("{mode:04o}")).with_location(Location {
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

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    fn script(mode: u32) -> TestFile {
        TestFile::new("#!/bin/sh\n").mode(mode)
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            // Standard setuid, setgid and both.
            tree(vec![("usr/bin/su", script(0o4755))]),
            tree(vec![("usr/bin/wall", script(0o2755))]),
            tree(vec![("usr/bin/both", script(0o6755))]),
            // The historic setgid exemption.
            tree(vec![("usr/bin/exempt", script(0o4754))]),
            // Neither bit: other Tags.
            tree(vec![("usr/bin/plain", script(0o744))]),
            tree(vec![("usr/bin/sticky", script(0o1755))]),
            // `root/games` executables that are not setgid with `0755`: the
            // game Tag, LN1792.
            tree(vec![(
                "usr/games/tool",
                script(0o4744).owned_by("root", "games"),
            )]),
            tree(vec![(
                "usr/games/tool",
                script(0o2750).owned_by("root", "games"),
            )]),
            // Directories and symlinks are not files.
            tree(vec![(
                "usr/share/doc/pkg",
                TestFile::directory().mode(0o2775),
            )]),
            tree(vec![("usr/bin/link", TestFile::symlink("su").mode(0o777))]),
        ];
        let fail = vec![
            tree(vec![("usr/bin/script-wuid", script(0o4744))]),
            tree(vec![("usr/bin/suidperlfoo", script(0o4555))]),
            tree(vec![("usr/bin/suid-wide", script(0o4775))]),
            // Setuid without an executable mode: the mode still differs.
            tree(vec![("usr/bin/suid-ro", script(0o4644))]),
            tree(vec![("usr/bin/hard", script(0o4700).hardlink())]),
            tree(vec![("bin/su", script(0o4750))]).udeb(),
            // Only `root/games` has the game Tag.
            tree(vec![(
                "usr/games/tool",
                script(0o4744).owned_by("root", "staff"),
            )]),
            tree(vec![(
                "usr/games/tool",
                script(0o2750).owned_by("alice", "games"),
            )]),
        ];
        BinaryPackageTester::new(NonStandardSetuidExecutablePerm, pass, fail).test_and_snapshot();
    }
}
