//! `debian/rules` as lintian's `debian/rules` Check reads it.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{EntryKind, FileTree};

static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("regex"));

/// `debian/rules` contents from the packaging File tree, following a
/// symlink. Missing files, directories, unreadable files, and non-UTF-8
/// contents are unavailable (lintian returns before scanning).
pub(super) fn read_rules_from_tree(files: &FileTree) -> Option<String> {
    let entry = files.get("debian/rules")?;
    match entry.kind() {
        EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink => {}
        EntryKind::Directory => return None,
    }
    let bytes = files.read(entry).ok()?;
    String::from_utf8(bytes).ok()
}

/// Logical lines: comment lines dropped, `\` continuations joined.
pub(super) fn complete_lines(contents: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut continued = String::new();
    for line in contents.split('\n') {
        if COMMENT.is_match(line) {
            continue;
        }
        let line = if continued.is_empty() {
            line.to_string()
        } else {
            let mut joined = std::mem::take(&mut continued);
            joined.push_str(line);
            joined
        };
        if let Some(stripped) = line.strip_suffix('\\') {
            continued = stripped.to_string();
            continue;
        }
        lines.push(line);
    }
    lines
}
