use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::owner_exceptions::is_game_executable_with_own_tag;

pub struct NonStandardExecutablePerm;

declare_rule! {
    /// An executable that is not setuid or setgid does not have mode 0755.
    ///
    /// Walks Data files regular files and hardlinks with any execute bit.
    /// Setuid and setgid files are left to the setuid Tags. Any other mode
    /// than `0755`, the sticky bit included, is reported with the extra
    /// `<mode> != 0755`. The path is the Location. Udebs are checked too.
    /// A `root/games` executable that is not setgid, or lacks a bit of
    /// `0755`, is skipped: lintian reports it as
    /// `non-standard-game-executable-perm` instead (LN1792 reports it, so this
    /// Rule gets no hint).
    NonStandardExecutablePerm,
    code = "LN0700",
    tag = "non-standard-executable-perm",
    default_selected = true,
    experimental = false,
}

const STANDARD_EXECUTABLE: u32 = 0o755;
const SETUID_OR_SETGID: u32 = 0o6000;

impl BinaryPackageRule for NonStandardExecutablePerm {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                entry.is_executable_file()
                    && entry.mode() & SETUID_OR_SETGID == 0
                    && entry.mode() != STANDARD_EXECUTABLE
                    && !is_game_executable_with_own_tag(entry)
            })
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("{mode:04o} != {STANDARD_EXECUTABLE:04o}")).with_location(
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

    fn script(mode: u32) -> TestFile {
        TestFile::new("#!/bin/sh\n").mode(mode)
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            tree(vec![("usr/bin/tool", script(0o755))]),
            // Not executable: other Tags.
            tree(vec![("usr/share/doc/pkg/README", script(0o644))]),
            tree(vec![("usr/share/doc/pkg/read-only", script(0o444))]),
            // Setuid and setgid: other Tags.
            tree(vec![("usr/bin/suid", script(0o4755))]),
            tree(vec![("usr/bin/suid-wrong", script(0o4744))]),
            tree(vec![("usr/bin/sgid", script(0o2750))]),
            // `root/games` executables that are not setgid with `0755`: the
            // game Tag, LN1792.
            tree(vec![(
                "usr/games/tool",
                script(0o750).owned_by("root", "games"),
            )]),
            tree(vec![(
                "usr/games/tool",
                script(0o775).owned_by("root", "games"),
            )]),
            // Directories and symlinks are not executable files.
            tree(vec![(
                "usr/share/doc/pkg",
                TestFile::directory().mode(0o744),
            )]),
            tree(vec![(
                "usr/bin/link",
                TestFile::symlink("tool").mode(0o777),
            )]),
        ];
        let fail = vec![
            tree(vec![("usr/bin/script-wexec", script(0o775))]),
            tree(vec![("etc/init.d/binary", script(0o765))]),
            tree(vec![("usr/bin/script-ro", script(0o751))]),
            tree(vec![("usr/bin/sticky", script(0o1755))]),
            tree(vec![("usr/bin/hard", script(0o700).hardlink())]),
            tree(vec![("bin/tool", script(0o750))]).udeb(),
            // Only `root/games` has the game Tag.
            tree(vec![(
                "usr/games/tool",
                script(0o750).owned_by("root", "staff"),
            )]),
            tree(vec![(
                "usr/games/tool",
                script(0o750).owned_by("alice", "games"),
            )]),
        ];
        BinaryPackageTester::new(NonStandardExecutablePerm, pass, fail).test_and_snapshot();
    }
}
