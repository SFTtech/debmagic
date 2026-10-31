//! Systemd unit text, as `systemd`'s service-file helpers read it.
//!
//! Continuation lines, comments, and empty assignments follow
//! `service_file_lines` and `extract_service_file_values`. Values are split
//! with shell words. No mode, owner, or size.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{EntryKind, FileTree};

static SERVICE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/systemd/system/.*\.service$").expect("valid regex"));
static SECTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[([^\]]+)\]$").expect("valid regex"));
/// First `=` splits the key. Whitespace immediately before `=` stays on the
/// key, as in lintian 2.139.0's greedy `^(.*)\s*=\s*(.*)$`, so `After =`
/// is not the key `After`.
static ASSIGNMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*?)(\s*)=\s*(.*)$").expect("valid regex"));

pub(in crate::lint::rules::lintian) fn service_texts(files: &FileTree) -> Vec<(PathBuf, String)> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .path(&SERVICE)
        .into_iter()
        .filter_map(|entry| {
            let text = read_text(files, entry)?;
            Some((entry.path().to_path_buf(), text))
        })
        .collect()
}

/// Basenames of `*.service` units (an optional `@` before `.service` dropped)
/// plus `Install` `Alias=` names (a trailing `.service` dropped).
pub(super) fn provided_service_names(files: &FileTree) -> HashSet<String> {
    let mut names = HashSet::new();
    for entry in files.query().path(&SERVICE) {
        let Some(base) = entry.path().file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(name) = unit_name(base) else {
            continue;
        };
        names.insert(name);
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            continue;
        }
        let Some(text) = read_text(files, entry) else {
            continue;
        };
        let lines = service_lines(&text);
        for alias in service_values(&lines, "Install", "Alias") {
            let alias = alias.strip_suffix(".service").unwrap_or(&alias);
            if !alias.is_empty() {
                names.insert(alias.to_string());
            }
        }
    }
    names
}

pub(in crate::lint::rules::lintian) fn service_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut continuation = String::new();
    for raw in text.split('\n') {
        let mut line = std::mem::take(&mut continuation);
        line.push_str(raw);
        if let Some(stripped) = line.strip_suffix('\\') {
            continuation = stripped.to_string();
            continuation.push(' ');
            continue;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        lines.push(trimmed.to_string());
    }
    lines
}

pub(in crate::lint::rules::lintian) fn service_values(
    lines: &[String],
    section: &str,
    key: &str,
) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = None;
    for line in lines {
        if let Some(caps) = SECTION.captures(line) {
            current = Some(caps[1].to_string());
            continue;
        }
        if current.as_deref() != Some(section) {
            continue;
        }
        let Some(caps) = ASSIGNMENT.captures(line) else {
            continue;
        };
        if &caps[1] != key || !caps[2].is_empty() {
            continue;
        }
        let value = &caps[3];
        if value.is_empty() {
            values.clear();
        } else {
            values.extend(shellwords(value));
        }
    }
    values
}

fn unit_name(basename: &str) -> Option<String> {
    let stem = basename.strip_suffix(".service")?;
    Some(stem.strip_suffix('@').unwrap_or(stem).to_string())
}

fn read_text(files: &FileTree, entry: &crate::lint::file_tree::Entry) -> Option<String> {
    files
        .read(entry)
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// `Text::ParseWords::shellwords` for one value: whitespace splits, quotes drop.
fn shellwords(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    while let Some(ch) = chars.next() {
        if let Some(mark) = quote {
            if ch == '\\' {
                if let Some(next) = chars.next() {
                    current.push(next);
                    in_word = true;
                }
                continue;
            }
            if ch == mark {
                quote = None;
                in_word = true;
                continue;
            }
            current.push(ch);
            in_word = true;
            continue;
        }
        match ch {
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                    in_word = true;
                }
            }
            '"' | '\'' => {
                quote = Some(ch);
                in_word = true;
            }
            ch if ch.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            ch => {
                current.push(ch);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(current);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_name_strips_template_suffix() {
        assert_eq!(unit_name("foo.service").as_deref(), Some("foo"));
        assert_eq!(unit_name("foo@.service").as_deref(), Some("foo"));
        assert_eq!(unit_name("foo@bar.service").as_deref(), Some("foo@bar"));
    }

    #[test]
    fn continuation_and_empty_assignment() {
        let lines = service_lines("[Unit]\nAfter=network.target \\\nsyslog.target\nAfter=\n");
        assert_eq!(
            service_values(&lines, "Unit", "After"),
            Vec::<String>::new()
        );
        let lines = service_lines("[Unit]\nAfter=\nAfter=syslog.target dbus.target\n");
        assert_eq!(
            service_values(&lines, "Unit", "After"),
            ["syslog.target", "dbus.target"]
        );
    }

    #[test]
    fn space_before_equals_is_part_of_the_key() {
        let lines = service_lines("[Unit]\nAfter = syslog.target\nAfter=dbus.target\n");
        assert_eq!(service_values(&lines, "Unit", "After"), ["dbus.target"]);
    }

    #[test]
    fn shellwords_drop_quotes() {
        assert_eq!(
            shellwords(r#""/etc/init.d/foo" start"#),
            ["/etc/init.d/foo", "start"]
        );
    }
}
