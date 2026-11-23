use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::owner_exceptions::is_game_executable_with_own_tag;

pub struct ExecutableIsNotWorldReadable;

declare_rule! {
    /// An executable file that is not readable by everyone.
    ///
    /// Walks Data files regular files and hardlinks with any execute bit.
    /// Reported with the octal mode as the extra when any of the read bits
    /// `0444` is missing, so `0751` or `4751` are reported. The path is the
    /// Location. Udebs are checked too. A `root/games` executable that is
    /// not setgid, or lacks a bit of `0755`, is skipped: lintian reports it as
    /// `non-standard-game-executable-perm` instead (no Rule for that Tag here,
    /// so it gets no hint).
    ExecutableIsNotWorldReadable,
    code = "LN0942",
    tag = "executable-is-not-world-readable",
    default_selected = true,
    experimental = false,
}

const WORLD_READABLE: u32 = 0o444;

impl BinaryPackageRule for ExecutableIsNotWorldReadable {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                entry.is_executable_file()
                    && entry.mode() & WORLD_READABLE != WORLD_READABLE
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
            // Readable by everyone.
            tree(vec![("usr/bin/tool", script(0o755))]),
            tree(vec![("usr/bin/tool-r", script(0o555))]),
            // Not executable: other Tags.
            tree(vec![("usr/share/doc/pkg/README", script(0o600))]),
            tree(vec![("usr/share/doc/pkg/read-only", script(0o444))]),
            // Setuid and setgid but readable: other Tags.
            tree(vec![("usr/bin/suid", script(0o4755))]),
            tree(vec![("usr/bin/sgid", script(0o2755))]),
            // `root/games` executables that are not setgid with `0755`: the
            // game Tag, LN1792.
            tree(vec![(
                "usr/games/tool",
                script(0o750).owned_by("root", "games"),
            )]),
            tree(vec![(
                "usr/games/tool",
                script(0o2751).owned_by("root", "games"),
            )]),
            // Directories and symlinks are not executable files.
            tree(vec![(
                "usr/share/doc/pkg",
                TestFile::directory().mode(0o700),
            )]),
            tree(vec![(
                "usr/bin/link",
                TestFile::symlink("tool").mode(0o777),
            )]),
        ];
        let fail = vec![
            // Missing the read bit of group or other.
            tree(vec![("usr/bin/script-ro", script(0o751))]),
            tree(vec![("usr/bin/script-no-group-r", script(0o705))]),
            tree(vec![("usr/bin/script-no-other-r", script(0o750))]),
            tree(vec![("usr/bin/owner-only", script(0o700))]),
            // Setuid executable that is not world-readable.
            tree(vec![("usr/bin/suidperlfoo2", script(0o4751))]),
            tree(vec![("usr/bin/hard", script(0o700).hardlink())]),
            tree(vec![("bin/tool", script(0o750))]).udeb(),
            // Only `root/games` has the game Tag.
            tree(vec![(
                "usr/games/tool",
                script(0o700).owned_by("root", "staff"),
            )]),
            tree(vec![(
                "usr/games/tool",
                script(0o700).owned_by("games", "games"),
            )]),
        ];
        BinaryPackageTester::new(ExecutableIsNotWorldReadable, pass, fail).test_and_snapshot();
    }
}
