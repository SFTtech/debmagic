//! md5sums control-file text from `Lintian::Check::Md5sums`.
//!
//! An empty file is skipped. A symlink is skipped. Paths lose one leading
//! `./`. Installed files are regular files and hardlinks.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Location;
use crate::lint::file_tree::EntryKind;

static LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^((?:\\)?\S{32})  (.*)$").expect("md5sums line"));
static ASPELL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^var/lib/[ai]spell/.").expect("aspell path"));

pub(super) struct Md5sumsFile {
    pub errors: Vec<String>,
    /// First-seen order. A repeated path keeps the later checksum.
    pub entries: Vec<(String, String)>,
}

pub(super) struct Installed {
    pub path: String,
    pub bytes: Option<Vec<u8>>,
}

pub(super) fn at() -> Location {
    Location {
        path: "md5sums".into(),
        line: None,
        column: None,
    }
}

/// `None` when the control file is missing, empty, a symlink, a directory,
/// or unreadable. Empty bytes are the skip; there is no separate size field.
pub(super) fn load(ctx: &mut BinaryPackageContext<'_>) -> Option<Md5sumsFile> {
    let files = ctx.control_files()?;
    let entry = files.get("md5sums")?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(parse(&bytes))
}

pub(super) fn parse(bytes: &[u8]) -> Md5sumsFile {
    let text = decode(bytes);
    let mut errors = Vec::new();
    let mut entries = Vec::new();
    for line in perl_lines(&text) {
        if line.is_empty() {
            continue;
        }
        let Some(caps) = LINE.captures(line) else {
            errors.push(format!("Odd text: {line}"));
            continue;
        };
        let mut checksum = caps[1].to_string();
        let raw_path = caps[2].to_string();
        if raw_path.is_empty() {
            errors.push(format!("Odd text: {line}"));
            continue;
        }
        let problematic = checksum.starts_with('\\');
        if problematic {
            checksum.remove(0);
        }
        let Ok(path) = unescape(&raw_path, problematic) else {
            errors.push(format!("Odd text: {line}"));
            continue;
        };
        if path.is_empty() {
            errors.push(format!("Empty name for checksum {checksum}"));
        }
        upsert(&mut entries, drop_relative_prefix(&path), checksum);
    }
    Md5sumsFile { errors, entries }
}

pub(super) fn installed_files(ctx: &mut BinaryPackageContext<'_>) -> Option<Vec<Installed>> {
    let files = ctx.data_files()?;
    Some(
        files
            .entries()
            .iter()
            .filter_map(|entry| {
                if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                    return None;
                }
                let path = entry.path().to_str()?.to_string();
                let bytes = files.read(entry).ok();
                Some(Installed { path, bytes })
            })
            .collect(),
    )
}

/// Paths from the conffiles control file, relative to the package root.
/// The filename is the last word. One leading `/` is removed.
pub(super) fn conffiles(ctx: &mut BinaryPackageContext<'_>) -> HashSet<String> {
    let Some(files) = ctx.control_files() else {
        return HashSet::new();
    };
    let Some(entry) = files.get("conffiles") else {
        return HashSet::new();
    };
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return HashSet::new();
    }
    let Ok(bytes) = files.read(entry) else {
        return HashSet::new();
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return HashSet::new();
    };
    text.lines()
        .filter_map(|line| {
            let path = line.split_whitespace().next_back()?;
            let path = path.strip_prefix('/').unwrap_or(path);
            (!path.is_empty()).then(|| path.to_string())
        })
        .collect()
}

pub(super) fn aspell_path(path: &str) -> bool {
    ASPELL.is_match(path)
}

fn upsert(entries: &mut Vec<(String, String)>, path: String, checksum: String) {
    if let Some((_, existing)) = entries.iter_mut().find(|(name, _)| name == &path) {
        *existing = checksum;
    } else {
        entries.push((path, checksum));
    }
}

fn drop_relative_prefix(path: &str) -> String {
    path.strip_prefix("./").unwrap_or(path).to_string()
}

fn unescape(path: &str, problematic: bool) -> Result<String, ()> {
    if !problematic {
        return Ok(path.to_string());
    }
    let mut out = String::new();
    let mut escaped = false;
    for character in path.chars() {
        if character == '\\' && !escaped {
            escaped = true;
            continue;
        }
        if character == 'n' && escaped {
            out.push('\n');
        } else {
            out.push(character);
        }
        escaped = false;
    }
    if escaped { Err(()) } else { Ok(out) }
}

fn decode(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec())
        .unwrap_or_else(|_| bytes.iter().map(|byte| char::from(*byte)).collect())
}

fn perl_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

pub(super) fn md5_hex(bytes: &[u8]) -> String {
    debmagic_common::checksums::Hash::Md5.hex(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaped_name_unescapes_newline() {
        let checksum = "d41d8cd98f00b204e9800998ecf8427e";
        let line = format!("\\{checksum}  a\\nb\n");
        let parsed = parse(line.as_bytes());
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert_eq!(
            parsed.entries,
            vec![("a\nb".to_string(), checksum.to_string())]
        );
    }

    #[test]
    fn dot_slash_is_stripped_once() {
        let checksum = "0cc175b9c0f1b6a831c399e269772661";
        let parsed = parse(format!("{checksum}  ./usr/bin/foo\n").as_bytes());
        assert_eq!(parsed.entries[0].0, "usr/bin/foo");
        let twice = parse(format!("{checksum}  ././usr/bin/foo\n").as_bytes());
        assert_eq!(twice.entries[0].0, "./usr/bin/foo");
    }
}
