use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Owner;
use crate::lint::rule::BinaryPackageRule;

use super::owner_exceptions::is_game_executable_with_own_tag;

pub struct ElevatedPrivileges;

declare_rule! {
    /// A file that runs with its owner's or group's identity: setuid or setgid.
    ///
    /// Walks Data files regular files and hardlinks with the setuid or setgid
    /// bit, executable or not. Reported with the extra `<mode> <owner>/<group>`
    /// (`4755 root/root`, mode in octal, owner and group as named in the
    /// package); the path is the Location. Udebs are checked too.
    ///
    /// Skipped, as lintian does: a package whose name contains `-suid`; a
    /// setuid, not setgid, file owned by `root` that is linked against
    /// `libvga.so.*` (svgalib); a setgid, not setuid, file of group `games`
    /// under `usr/games/` or `usr/lib/games/`; and a `root/games` executable
    /// that lintian reports as `non-standard-game-executable-perm` (no Rule
    /// for that Tag here). An Entry with no owner would be shown as
    /// `root/root`, lintian's default; Binary package Entries always have one.
    ElevatedPrivileges,
    code = "LN0997",
    tag = "elevated-privileges",
    default_selected = true,
    experimental = false,
}

const SETUID: u32 = 0o4000;
const SETGID: u32 = 0o2000;

/// A setuid or setgid file, with what the exceptions look at.
struct Candidate {
    path: PathBuf,
    mode: u32,
    identity: String,
    owned_by_root: bool,
    group_is_games: bool,
}

impl BinaryPackageRule for ElevatedPrivileges {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name().contains("-suid") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let candidates: Vec<Candidate> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_file() && entry.mode() & (SETUID | SETGID) != 0)
            .filter(|entry| !is_game_executable_with_own_tag(entry))
            .map(|entry| {
                // Lintian's default for an Entry without an owner.
                let owner = entry.owner().cloned().unwrap_or_else(Owner::root);
                Candidate {
                    path: entry.path().to_path_buf(),
                    mode: entry.mode(),
                    identity: owner.identity(),
                    owned_by_root: owner.user() == "root",
                    group_is_games: owner.group() == "games",
                }
            })
            .collect();
        if candidates.is_empty() {
            return;
        }
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for candidate in candidates {
            let setuid = candidate.mode & SETUID != 0;
            let setgid = candidate.mode & SETGID != 0;
            let svgalib = setuid
                && !setgid
                && candidate.owned_by_root
                && elf_files.get(&candidate.path).is_some_and(|facts| {
                    facts
                        .needed()
                        .iter()
                        .any(|library| library.starts_with("libvga.so."))
                });
            let game =
                setgid && !setuid && candidate.group_is_games && is_games_program(&candidate.path);
            if svgalib || game {
                continue;
            }
            ctx.diagnostic(
                Diagnostic::warn(format!("{:04o} {}", candidate.mode, candidate.identity))
                    .with_location(Location {
                        path: candidate.path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// Lintian's `^ usr/ (?:lib/)? games/ \S+`.
fn is_games_program(path: &std::path::Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let path = path.as_os_str().as_bytes();
    [&b"usr/games/"[..], &b"usr/lib/games/"[..]]
        .iter()
        .filter_map(|prefix| path.strip_prefix(*prefix))
        .any(|rest| rest.first().is_some_and(|byte| !byte.is_ascii_whitespace()))
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    fn script(mode: u32) -> TestFile {
        TestFile::new("#!/bin/sh\n").mode(mode)
    }

    fn elf(mode: u32, needed: &str) -> TestFile {
        TestFile::from_bytes(TestElf::executable().needed(needed).build()).mode(mode)
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            tree(vec![("usr/bin/plain", script(0o755))]),
            tree(vec![("usr/bin/sticky", script(0o1755))]),
            // Directories carry the bits without running anything.
            tree(vec![(
                "usr/share/doc/pkg",
                TestFile::directory().mode(0o2775),
            )]),
            // A package named `-suid` is meant to have them.
            tree(vec![("usr/bin/su", script(0o4755))]).package("busybox-suid"),
            // Setuid root programs that need svgalib.
            tree(vec![("usr/bin/vga", elf(0o4755, "libvga.so.1"))]),
            // Setgid games.
            tree(vec![(
                "usr/games/tool",
                script(0o2755).owned_by("root", "games"),
            )]),
            tree(vec![(
                "usr/lib/games/tool",
                script(0o2755).owned_by("alice", "games"),
            )]),
            // `root/games` executables that lintian reports as game Tags.
            tree(vec![(
                "usr/games/tool",
                script(0o4755).owned_by("root", "games"),
            )]),
            tree(vec![(
                "usr/bin/tool",
                script(0o2750).owned_by("root", "games"),
            )]),
        ];
        let fail = vec![
            tree(vec![("usr/bin/script-uid", script(0o4755))]),
            tree(vec![("usr/bin/script-gid", script(0o2755))]),
            tree(vec![("usr/bin/script-ugid", script(0o6755))]),
            tree(vec![("usr/bin/script-wuid", script(0o4744))]),
            tree(vec![("usr/bin/data", script(0o4644))]),
            tree(vec![("usr/bin/hard", script(0o4755).hardlink())]),
            tree(vec![("bin/su", script(0o4755))]).udeb(),
            tree(vec![(
                "usr/bin/owned",
                script(0o4755).owned_by("bin", "bin"),
            )]),
            // Not an svgalib program: another library, or setgid too, or not root.
            tree(vec![("usr/bin/other", elf(0o4755, "libc.so.6"))]),
            tree(vec![("usr/bin/vga-gid", elf(0o6755, "libvga.so.1"))]),
            tree(vec![(
                "usr/bin/vga-user",
                elf(0o4755, "libvga.so.1").owned_by("alice", "root"),
            )]),
            // Not a setgid game: setuid too, another group, or another place.
            tree(vec![(
                "usr/games/both",
                script(0o6755).owned_by("root", "staff"),
            )]),
            tree(vec![(
                "usr/games/tool",
                script(0o2755).owned_by("root", "staff"),
            )]),
            tree(vec![(
                "usr/bin/tool",
                script(0o2755).owned_by("root", "games"),
            )]),
            tree(vec![(
                "usr/games",
                TestFile::new("x\n").mode(0o2755).owned_by("root", "games"),
            )]),
            tree(vec![("usr/bin/su", script(0o4755))]).package("su-helper"),
        ];
        BinaryPackageTester::new(ElevatedPrivileges, pass, fail).test_and_snapshot();
    }
}
