//! Shared control-file text helpers for the `debian/shlibs` Check.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;

static BLANK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*$").expect("blank line"));

/// Perl `split(/\n/, ...)` : trailing empty fields are dropped, blank lines in
/// the middle stay, and a final newline does not invent an extra line.
pub(super) fn perl_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

pub(super) fn is_blank_or_comment(line: &str) -> bool {
    BLANK.is_match(line) || line.starts_with('#')
}

/// Perl `split` with a pattern: keep a leading or middle empty field, drop
/// trailing empty fields.
pub(super) fn perl_split<'a>(pattern: &Regex, text: &'a str) -> Vec<&'a str> {
    let mut parts: Vec<&str> = pattern.split(text).collect();
    while parts.last().is_some_and(|part| part.is_empty()) {
        parts.pop();
    }
    parts
}

pub(super) fn read_text(ctx: &mut BinaryPackageContext<'_>, name: &str) -> Option<String> {
    let files = ctx.control_files()?;
    let entry = files.get(name)?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// `(lines 1 2) pretty`, with line numbers sorted ascending.
pub(super) fn duplicate_groups(
    positions: &HashMap<String, Vec<u32>>,
    pretty: impl Fn(&str) -> String,
) -> Vec<String> {
    let mut groups = Vec::new();
    for (name, lines) in positions {
        if lines.len() < 2 {
            continue;
        }
        let first = lines.iter().copied().min().unwrap_or(0);
        let mut sorted = lines.clone();
        sorted.sort_unstable();
        let mut message = String::from("(lines");
        for line in sorted {
            message.push(' ');
            message.push_str(&line.to_string());
        }
        message.push_str(") ");
        message.push_str(&pretty(name));
        groups.push((first, message));
    }
    groups.sort_by_key(|(first, _)| *first);
    groups.into_iter().map(|(_, message)| message).collect()
}
