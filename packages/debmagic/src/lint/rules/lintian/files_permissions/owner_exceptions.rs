//! The owner-dependent exceptions of lintian's `files/permissions` Check,
//! shared by the Rules of this module.
//!
//! An Entry without an owner (see [`crate::lint::file_tree::Owner`]) is never
//! `root/games`, `root/staff` or `root/src`, so none of these exceptions
//! applies to it. Binary package Entries always have an owner.

use crate::lint::file_tree::Entry;

const SETGID: u32 = 0o2000;
const STANDARD_EXECUTABLE: u32 = 0o755;

/// A `root/games` executable that lintian reports as
/// `non-standard-game-executable-perm` and then skips for every other
/// permission Tag: it is not setgid, or lacks a bit of `0755`.
///
/// LN1792 reports these, so the other permission Rules leave them alone.
pub(super) fn is_game_executable_with_own_tag(entry: &Entry) -> bool {
    entry.is_executable_file()
        && entry.has_identity("root/games")
        && (entry.mode() & SETGID == 0 || entry.mode() & STANDARD_EXECUTABLE != STANDARD_EXECUTABLE)
}

/// Lintian's `^ var/ (?:lib/)? games/ \S+` on a path: below `var/games/` or
/// `var/lib/games/`, the next character not whitespace. `path` is the stored
/// path, without the trailing `/` that lintian adds to directories; a
/// directory still matches when something follows `games/`.
pub(super) fn is_var_games_path(path: &[u8]) -> bool {
    [&b"var/games/"[..], &b"var/lib/games/"[..]]
        .iter()
        .filter_map(|prefix| path.strip_prefix(*prefix))
        .any(|rest| rest.first().is_some_and(|byte| !byte.is_ascii_whitespace()))
}

#[cfg(test)]
mod tests {
    use crate::lint::file_tree::Owner;

    use super::*;

    fn game_script(mode: u32, owner: Option<Owner>) -> Entry {
        let entry = Entry::regular_file("usr/games/tool", None, None).with_mode(mode);
        match owner {
            Some(owner) => entry.with_owner(owner),
            None => entry,
        }
    }

    #[test]
    fn game_executable_has_its_own_tag_unless_setgid_and_standard() {
        let games = || Some(Owner::named(0, 60, "root", "games"));
        assert!(is_game_executable_with_own_tag(&game_script(
            0o755,
            games()
        )));
        assert!(is_game_executable_with_own_tag(&game_script(
            0o2750,
            games()
        )));
        assert!(is_game_executable_with_own_tag(&game_script(
            0o4755,
            games()
        )));
        assert!(!is_game_executable_with_own_tag(&game_script(
            0o2755,
            games()
        )));
        assert!(!is_game_executable_with_own_tag(&game_script(
            0o6755,
            games()
        )));
        // Not executable: nothing to report as a game executable.
        assert!(!is_game_executable_with_own_tag(&game_script(
            0o664,
            games()
        )));
        // Another owner, another group, or no owner at all.
        assert!(!is_game_executable_with_own_tag(&game_script(
            0o755,
            Some(Owner::root())
        )));
        assert!(!is_game_executable_with_own_tag(&game_script(
            0o755,
            Some(Owner::named(1000, 60, "alice", "games"))
        )));
        assert!(!is_game_executable_with_own_tag(&game_script(0o755, None)));
    }

    #[test]
    fn var_games_path_needs_something_after_games() {
        assert!(is_var_games_path(b"var/games/pkg"));
        assert!(is_var_games_path(b"var/lib/games/pkg/score"));
        assert!(!is_var_games_path(b"var/games"));
        assert!(!is_var_games_path(b"var/lib/games"));
        assert!(!is_var_games_path(b"var/games/ x"));
        assert!(!is_var_games_path(b"usr/games/pkg"));
        assert!(!is_var_games_path(b"var/lib/pkg"));
    }
}
