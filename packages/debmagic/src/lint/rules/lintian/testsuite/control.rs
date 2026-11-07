//! `debian/tests/control` as lintian reads it.
//!
//! `folded_paragraphs` unfolds continuation lines the way the python-version
//! check splits `Tests:`. `paragraphs` is Deb822, the way the no-op testsuite
//! check counts sections.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use debian_control::lossless::Control;
use regex::Regex;

use crate::lint::file_tree::{EntryKind, FileTree};

#[derive(Default)]
pub(super) struct Paragraph {
    fields: Vec<(String, String)>,
}

impl Paragraph {
    pub(super) fn unfolded(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

static TEST_SPLIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*,\s*|\s+").expect("valid regex"));

fn folded_paragraphs(text: &str) -> Vec<Paragraph> {
    let mut paragraphs = Vec::new();
    let mut current = Paragraph::default();
    for raw in text.split('\n') {
        let line = raw.trim_end_matches('\r');
        if line.is_empty() {
            if !current.fields.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
            continue;
        }
        if line.starts_with([' ', '\t']) {
            if let Some((_, value)) = current.fields.last_mut() {
                value.push(' ');
                value.push_str(line.trim());
            }
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            current
                .fields
                .push((name.to_string(), value.trim().to_string()));
        }
    }
    if !current.fields.is_empty() {
        paragraphs.push(current);
    }
    paragraphs
}

/// Test script paths named by every `Tests:` field.
///
/// Names are unique within one paragraph, in the order lintian's `uniq` keeps.
pub(super) fn test_paths(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for paragraph in folded_paragraphs(text) {
        let Some(tests) = paragraph.unfolded("Tests") else {
            continue;
        };
        let directory = paragraph
            .unfolded("Tests-Directory")
            .unwrap_or("debian/tests");
        let mut seen = Vec::new();
        for name in TEST_SPLIT.split(tests) {
            if name.is_empty() || seen.contains(&name) {
                continue;
            }
            seen.push(name);
            if directory == "." {
                paths.push(name.to_string());
            } else {
                paths.push(format!("{directory}/{name}"));
            }
        }
    }
    paths
}

pub(super) fn read_tests_control(files: &FileTree) -> Option<String> {
    let entry = files.get(Path::new("debian/tests/control"))?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    String::from_utf8(bytes).ok()
}

pub(super) fn read_regular(files: &FileTree, path: &str) -> Option<(PathBuf, String)> {
    let entry = files.get(Path::new(path))?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Some((entry.path().to_path_buf(), text))
}

/// Paragraphs of a tests control file. `None` when the text is not Deb822.
pub(super) fn paragraphs(text: &str) -> Option<Vec<Vec<(String, String)>>> {
    let control: Control = text.parse().ok()?;
    let paragraphs = control
        .as_deb822()
        .paragraphs()
        .map(|paragraph| {
            paragraph
                .entries()
                .filter_map(|entry| Some((entry.key()?.to_string(), entry.value().to_string())))
                .collect()
        })
        .collect();
    Some(paragraphs)
}

pub(super) fn field<'a>(paragraph: &'a [(String, String)], name: &str) -> Option<&'a str> {
    paragraph
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// `debian/tests` is a directory and `debian/tests/control` exists.
pub(super) fn tests_control_present(files: &FileTree) -> bool {
    let Some(directory) = files.get("debian/tests") else {
        return false;
    };
    if directory.kind() != EntryKind::Directory {
        return false;
    }
    files.get("debian/tests/control").is_some()
}

pub(super) fn read(files: &FileTree, path: &str) -> Option<String> {
    let entry = files.get(path)?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    String::from_utf8(bytes).ok()
}
