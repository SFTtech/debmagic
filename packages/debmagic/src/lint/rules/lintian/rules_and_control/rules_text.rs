//! `debian/rules` readers for the two ways lintian walks that file.
//!
//! The `debian/rules` Check drops comment lines, then joins `\` continuations,
//! and points at the last physical line. The `debhelper` Check joins
//! continuations first (keeping the newline), then drops comments, and points
//! at the first physical line. Continuation lines it consumes do not advance
//! its line counter, so a later hint can sit on an earlier number than the
//! physical line.

use std::path::{Component, Path, PathBuf};

use regex::Regex;
use std::sync::LazyLock;

use crate::lint::file_tree::{EntryKind, FileTree};

static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("regex"));

/// UTF-8 contents of `path`, following a relative symlink up to eight hops.
/// Missing, directories, unreadable, absolute links, and non-UTF-8 are
/// unavailable.
pub(super) fn read_utf8(files: &FileTree, path: &str) -> Option<String> {
    let bytes = read_bytes(files, Path::new(path))?;
    String::from_utf8(bytes).ok()
}

pub(super) fn read_rules(files: &FileTree) -> Option<String> {
    read_utf8(files, "debian/rules")
}

/// Logical lines of the `debian/rules` Check. The number is the last physical
/// line of the logical line.
pub(super) fn rules_lines(contents: &str) -> Vec<(u32, String)> {
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

/// Logical lines of the `debhelper` Check. Comment lines are absent. The
/// number is lintian's pointer, which lags after a continuation.
pub(super) fn debhelper_lines(contents: &str) -> Vec<(u32, String)> {
    let physical = perl_lines(contents);
    let mut lines = Vec::new();
    let mut index = 0;
    let mut position = 1u32;
    while index < physical.len() {
        let number = position;
        let mut line = physical[index].clone();
        index += 1;
        while let Some(stripped) = strip_backslash_before_end(&line) {
            if index >= physical.len() {
                line = stripped;
                break;
            }
            line = stripped;
            line.push_str(&physical[index]);
            index += 1;
        }
        if let Some(without_newline) = line.strip_suffix('\n') {
            line = without_newline.to_string();
        }
        if !COMMENT.is_match(&line) {
            lines.push((number, line));
        }
        position = position.saturating_add(1);
    }
    lines
}

/// Readable UTF-8 files under `debian/`, path order. Symlinks are followed.
pub(super) fn debian_texts(files: &FileTree) -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    for entry in files.entries() {
        let path = entry.path();
        if !path.starts_with("debian") {
            continue;
        }
        if entry.kind() == EntryKind::Directory {
            continue;
        }
        let Some(text) = read_utf8(files, &path.to_string_lossy()) else {
            continue;
        };
        found.push((path.to_path_buf(), text));
    }
    found
}

fn perl_lines(contents: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rest = contents;
    while let Some(index) = rest.find('\n') {
        let (line, after) = rest.split_at(index + 1);
        lines.push(line.to_string());
        rest = after;
    }
    if !rest.is_empty() {
        lines.push(rest.to_string());
    }
    lines
}

/// Perl `s/\\$//`: drop one backslash that sits at the end or just before a
/// final newline, and keep that newline.
fn strip_backslash_before_end(line: &str) -> Option<String> {
    if let Some(without_newline) = line.strip_suffix('\n') {
        let stripped = without_newline.strip_suffix('\\')?;
        let mut out = stripped.to_string();
        out.push('\n');
        return Some(out);
    }
    line.strip_suffix('\\').map(str::to_string)
}

fn read_bytes(files: &FileTree, path: &Path) -> Option<Vec<u8>> {
    let mut path = path.to_path_buf();
    for _ in 0..8 {
        let entry = files.get(&path)?;
        match entry.kind() {
            EntryKind::Directory => return None,
            EntryKind::RegularFile | EntryKind::Hardlink => return files.read(entry).ok(),
            EntryKind::Symlink => {
                let target = entry.link_target()?;
                path = resolve_link(&path, target)?;
            }
        }
    }
    None
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_lines_skip_comments_and_point_at_the_last_physical_line() {
        let lines = rules_lines("foo \\\n# comment\nbar\n");
        assert_eq!(lines, vec![(3, "foo bar".to_string()), (4, String::new())]);
    }

    #[test]
    fn debhelper_lines_join_before_comments_and_lag_the_counter() {
        let lines = debhelper_lines(
            "\tdh $@ \\\n\t--with autotools-dev\n\tdh_autotools-dev_updateconfig\n",
        );
        assert_eq!(
            lines,
            vec![
                (1, "\tdh $@ \n\t--with autotools-dev".to_string()),
                (2, "\tdh_autotools-dev_updateconfig".to_string()),
            ]
        );
    }

    #[test]
    fn debhelper_lines_drop_a_comment_that_swallows_a_continuation() {
        let lines = debhelper_lines("# comment \\\n\tdh_autotools-dev_updateconfig\n");
        assert!(lines.is_empty());
    }
}
