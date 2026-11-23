//! `debian/tests/control` sections with line numbers, the way lintian's
//! Deb822 parser reads them with comments at end of line, and symbolic link
//! resolution inside the tree.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::diagnostic::Location;
use crate::lint::file_tree::{Entry, FileTree};

use super::control::{TEST_SPLIT, read_tests_control};

/// Entry at `path` after following symbolic links inside the tree (lintian's
/// `resolve_path`). `None` when the path is missing or the links loop.
pub(super) fn resolve<'a>(files: &'a FileTree, path: &str) -> Option<&'a Entry> {
    let mut pending: VecDeque<String> = components(path).collect();
    let mut resolved: Vec<String> = Vec::new();
    let mut links = 0;
    while let Some(component) = pending.pop_front() {
        match component.as_str() {
            "." => continue,
            ".." => {
                resolved.pop();
                continue;
            }
            _ => {}
        }
        resolved.push(component);
        let Some(target) = files
            .get(resolved.join("/"))
            .and_then(|entry| entry.link_target())
        else {
            continue;
        };
        links += 1;
        if links > 40 {
            return None;
        }
        resolved.pop();
        let target = target.to_string_lossy().into_owned();
        if target.starts_with('/') {
            resolved.clear();
        }
        for component in components(&target).collect::<Vec<_>>().into_iter().rev() {
            pending.push_front(component);
        }
    }
    if resolved.is_empty() {
        return None;
    }
    files.get(resolved.join("/"))
}

fn components(path: &str) -> impl Iterator<Item = String> + '_ {
    path.split('/')
        .filter(|component| !component.is_empty())
        .map(str::to_string)
}

struct SectionField {
    name: String,
    line: u32,
    value: String,
}

/// A paragraph of a tests control file.
pub(super) struct Section {
    start: u32,
    fields: Vec<SectionField>,
}

static FIELD_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^: \t]+):\s*(.*)$").expect("valid regex"));

/// Sections of a tests control file. `None` when lintian would die on the text.
pub(super) fn sections(text: &str) -> Option<Vec<Section>> {
    let mut sections: Vec<Section> = Vec::new();
    let mut current: Option<Section> = None;
    for (index, raw) in text.lines().enumerate() {
        let line = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if raw.starts_with('#') {
            continue;
        }
        if raw.trim().is_empty() {
            sections.extend(current.take());
            continue;
        }
        if let Some(captures) = FIELD_LINE.captures(raw) {
            let section = current.get_or_insert_with(|| Section {
                start: line,
                fields: Vec::new(),
            });
            let name = captures[1].to_string();
            if section.fields.iter().any(|field| field.name == name) {
                return None;
            }
            let value = strip_comment(captures[2].trim_end());
            section.fields.push(SectionField { name, line, value });
        } else if raw.starts_with([' ', '\t']) {
            let field = current.as_mut()?.fields.last_mut()?;
            field.value.push('\n');
            field.value.push_str(&strip_comment(raw.trim_end()));
        } else {
            return None;
        }
    }
    sections.extend(current);
    Some(sections)
}

fn strip_comment(value: &str) -> String {
    value.split('#').next().unwrap_or("").to_string()
}

impl Section {
    /// Line of the first field: lintian's start of paragraph.
    pub(super) fn start(&self) -> u32 {
        self.start
    }

    pub(super) fn declares(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    fn find(&self, name: &str) -> Option<&SectionField> {
        self.fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(name))
    }

    /// Every field name as written, with its line, in file order.
    pub(super) fn field_names(&self) -> impl Iterator<Item = (&str, u32)> {
        self.fields
            .iter()
            .map(|field| (field.name.as_str(), field.line))
    }

    /// Line of the named field.
    pub(super) fn line(&self, name: &str) -> Option<u32> {
        self.find(name).map(|field| field.line)
    }

    /// Lintian's `unfolded_value`: line breaks removed, leading space cut. An
    /// undeclared field is empty.
    pub(super) fn unfolded_value(&self, name: &str) -> String {
        self.find(name)
            .map(|field| field.value.replace('\n', "").trim_start().to_string())
            .unwrap_or_default()
    }

    /// `Tests` split on commas and whitespace, without repeats, in order.
    pub(super) fn unique_tests(&self) -> Vec<String> {
        let value = self
            .find("Tests")
            .map(|field| field.value.trim())
            .unwrap_or("");
        let mut seen: Vec<String> = Vec::new();
        for name in TEST_SPLIT.split(value).map(str::trim) {
            if !name.is_empty() && !seen.iter().any(|known| known == name) {
                seen.push(name.to_string());
            }
        }
        seen
    }

    /// Paths of the unique tests under `Tests-Directory`.
    pub(super) fn test_file_paths(&self) -> Vec<String> {
        let directory = self.unfolded_value("Tests-Directory");
        let directory = if directory.is_empty() {
            "debian/tests".to_string()
        } else {
            directory
        };
        self.unique_tests()
            .into_iter()
            .map(|name| {
                if directory == "." {
                    name
                } else {
                    format!("{directory}/{name}")
                }
            })
            .collect()
    }
}

/// Location in `debian/tests/control`.
pub(super) fn control_location(line: Option<u32>) -> Location {
    Location {
        path: PathBuf::from("debian/tests/control"),
        line,
        column: None,
    }
}

/// Line of the first `name:` field in a control file text.
pub(super) fn field_line(text: &str, name: &str) -> Option<u32> {
    let prefix = format!("{}:", name.to_ascii_lowercase());
    text.lines().enumerate().find_map(|(index, line)| {
        line.to_ascii_lowercase()
            .starts_with(&prefix)
            .then_some(u32::try_from(index + 1).unwrap_or(u32::MAX))
    })
}

/// Sections of `debian/tests/control`, or `None` when it is absent or unparsable.
pub(super) fn tests_control_sections(files: &FileTree) -> Option<Vec<Section>> {
    sections(&read_tests_control(files)?)
}
