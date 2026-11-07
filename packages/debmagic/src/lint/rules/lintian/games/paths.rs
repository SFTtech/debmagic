//! Direct children of `bin`, `usr/bin`, and `usr/games`.

use crate::lint::file_tree::FileTree;

/// Entries whose parent is `directory`. The directory itself does not count.
pub(super) fn child_count(files: &FileTree, directory: &str) -> usize {
    files
        .entries()
        .iter()
        .filter(|entry| {
            let Some(path) = entry.path().to_str() else {
                return false;
            };
            let path = path.trim_end_matches('/');
            let Some(parent) = std::path::Path::new(path)
                .parent()
                .and_then(|parent| parent.to_str())
            else {
                return false;
            };
            parent == directory
        })
        .count()
}

pub(super) struct GameBins {
    pub other: usize,
    pub games: usize,
}

pub(super) fn game_bins(files: &FileTree) -> GameBins {
    GameBins {
        other: child_count(files, "bin") + child_count(files, "usr/bin"),
        games: child_count(files, "usr/games"),
    }
}
