//! `debian/rules` as lintian's `debian/rules` Check reads it.

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::diagnostic::Location;
use crate::lint::file_tree::{EntryKind, FileTree};

static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("regex"));

/// `debian/rules` contents from the packaging File tree, following a
/// symlink. Missing files, directories, unreadable files, and non-UTF-8
/// contents are unavailable (lintian returns before scanning).
pub(in super::super) fn read_rules_from_tree(files: &FileTree) -> Option<String> {
    let entry = files.get("debian/rules")?;
    match entry.kind() {
        EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink => {}
        EntryKind::Directory => return None,
    }
    let bytes = files.read(entry).ok()?;
    String::from_utf8(bytes).ok()
}

/// Logical lines that come before any Policy target has been seen (lintian's
/// `unless (%seen)` guard for settings it only wants outside rules).
pub(in super::super) fn lines_outside_targets(contents: &str) -> Vec<(u32, String)> {
    let scan = super::ln0003_debian_rules_missing_required_target::scan_policy_targets(contents);
    complete_lines(contents)
        .into_iter()
        .zip(scan.seen_before)
        .filter_map(|(line, seen)| (!seen).then_some(line))
        .collect()
}

/// `debian/rules` at the physical line a hint points to.
pub(in super::super) fn rules_location(line: u32) -> Location {
    Location {
        path: PathBuf::from("debian/rules"),
        line: Some(line),
        column: None,
    }
}

/// Logical lines with their line number: comment lines dropped, `\`
/// continuations joined. The number is that of the last physical line, which
/// is where lintian points.
pub(in super::super) fn complete_lines(contents: &str) -> Vec<(u32, String)> {
    let mut lines = Vec::new();
    let mut continued = String::new();
    for (index, line) in contents.split('\n').enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
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
        lines.push((number, line));
    }
    lines
}
