//! `debian/rules` as lintian's `debian/rules` Check reads it.

use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::diagnostic::Location;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};

static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("regex"));

/// `debian/rules` after following symbolic links, as lintian's `follow` does:
/// the path the chain resolves to and its entry. `None` when the chain ends
/// at a directory or special file, at an absolute, missing, or unresolvable
/// link, or after eight hops. Lintian judges the followed item, so the
/// Location of a hint on a link is the target.
pub(in super::super) fn follow_rules(files: &FileTree) -> Option<(PathBuf, &Entry)> {
    let mut path = PathBuf::from("debian/rules");
    for _ in 0..8 {
        let entry = files.get(&path)?;
        match entry.kind() {
            EntryKind::Symlink => path = resolve_link(&path, entry.link_target()?)?,
            EntryKind::Directory
            | EntryKind::Fifo
            | EntryKind::CharDevice
            | EntryKind::BlockDevice => return None,
            EntryKind::RegularFile | EntryKind::Hardlink => return Some((path, entry)),
        }
    }
    None
}

/// A relative link resolved against the directory of `from`.
fn resolve_link(from: &Path, target: &Path) -> Option<PathBuf> {
    if target.is_absolute() {
        return None;
    }
    let mut resolved = from.parent().unwrap_or(Path::new("")).to_path_buf();
    for component in target.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(resolved)
}

/// `debian/rules` contents from the packaging File tree, following a
/// symlink. Missing files, directories, unreadable files, and non-UTF-8
/// contents are unavailable (lintian returns before scanning).
pub(in super::super) fn read_rules_from_tree(files: &FileTree) -> Option<String> {
    let entry = files.get("debian/rules")?;
    match entry.kind() {
        EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink => {}
        EntryKind::Directory | EntryKind::Fifo | EntryKind::CharDevice | EntryKind::BlockDevice => {
            return None;
        }
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
