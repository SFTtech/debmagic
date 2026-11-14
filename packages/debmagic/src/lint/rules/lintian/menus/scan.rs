//! Shared walks for lintian's `menus` Check.
//!
//! Menu files, menu methods, and doc-base files are regular files and
//! hardlinks. Maintainer scripts are Control files read as text: there is no
//! ELF classification, and a symlink script is ignored the way lintian skips
//! it. Doc-base bytes that are not UTF-8 are skipped. File mode is not
//! available, so an executable doc-base file is still parsed.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{Entry, EntryKind, FileTree};

static MENU_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/(?:lib|share)/menu/\S").expect("valid regex"));
static MENU_METHOD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/menu-methods/\S").expect("valid regex"));
static UPDATE_MENUS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^\s*|[;&|]\s*|(?:then|do|exec)\s+)(?:/usr/bin/)?update-menus(?:\s|[;&|<>]|\z)")
        .expect("valid regex")
});
static DROP_X_INSTALL_DOCS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-x\s+\S*install-docs").expect("valid regex"));
static WHICH_INSTALL_DOCS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:which|type)\s+install-docs").expect("valid regex"));
static DROP_COMMAND_INSTALL_DOCS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"command\s+.*?install-docs").expect("valid regex"));
static CALL_INSTALL_DOCS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^\s*|[;&|]\s*|(?:then|do)\s+)(?:/usr/sbin/)?install-docs(?:\s|[;&|<>]|\z)")
        .expect("valid regex")
});
static REMOVE_INSTALL_DOCS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"install-docs\s+(?:-r|--remove)\s").expect("valid regex"));
static DOC_FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\S+)\s*:\s*(.*)$").expect("valid regex"));
static DOC_CONTINUATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+\S").expect("valid regex"));
static DOC_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*$").expect("valid regex"));
static ABSTRACT_DOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s+)\.(\s*)$").expect("valid regex"));

/// Last non-remove `install-docs` line and last `-r` / `--remove` line.
pub(super) struct InstallDocsCalls {
    pub add: Option<u32>,
    pub remove: Option<u32>,
}

struct OpenField {
    name: String,
    last_line: u32,
    vals: Vec<String>,
}

pub(super) struct DocBaseFile {
    pub path: PathBuf,
    pub no_format: bool,
    pub abstract_separators: Vec<u32>,
    /// `(line, field name)` of a field that is not known in its section.
    pub unknown_fields: Vec<(u32, String)>,
}

/// Menu file lintian would point at, otherwise the last menu method.
///
/// `usr/lib/menu/README` and `usr/share/menu/README` are not menu files. When
/// several menu files exist, the last path in sorted order wins.
pub(super) fn menu_anchor(files: &FileTree) -> Option<PathBuf> {
    let menus: Vec<_> = file_paths(files, &MENU_FILE)
        .into_iter()
        .filter(|path| !is_menu_readme(path))
        .collect();
    if let Some(path) = menus.last() {
        return Some(path.clone());
    }
    menu_method_paths(files).last().cloned()
}

pub(super) fn menu_method_paths(files: &FileTree) -> Vec<PathBuf> {
    file_paths(files, &MENU_METHOD)
}

pub(super) fn lacks_menu_h(files: &FileTree, path: &Path) -> bool {
    let Some(entry) = files.get(path) else {
        return true;
    };
    let Ok(bytes) = files.read(entry) else {
        return true;
    };
    let text = String::from_utf8_lossy(&bytes);
    !physical_lines(&text)
        .into_iter()
        .any(|line| line.trim_end_matches('\r').starts_with("!include menu.h"))
}

pub(super) fn calls_update_menus(files: &FileTree, name: &str) -> bool {
    let Some(text) = control_text(files, name) else {
        return false;
    };
    physical_lines(&text)
        .into_iter()
        .any(|line| UPDATE_MENUS.is_match(strip_hash_comment(line)))
}

pub(super) fn install_docs_calls(files: &FileTree, name: &str) -> InstallDocsCalls {
    let mut calls = InstallDocsCalls {
        add: None,
        remove: None,
    };
    let Some(text) = control_text(files, name) else {
        return calls;
    };
    for (index, raw) in physical_lines(&text).into_iter().enumerate() {
        let line_no = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let mut line = strip_hash_comment(raw).to_string();
        // lintian deletes the existence probe before it looks for the call.
        if DROP_X_INSTALL_DOCS.is_match(&line) {
            line = DROP_X_INSTALL_DOCS.replace(&line, "").into_owned();
        } else if !WHICH_INSTALL_DOCS.is_match(&line) {
            line = DROP_COMMAND_INSTALL_DOCS.replace(&line, "").into_owned();
        }
        if !CALL_INSTALL_DOCS.is_match(&line) {
            continue;
        }
        if REMOVE_INSTALL_DOCS.is_match(&line) {
            calls.remove = Some(line_no);
        } else {
            calls.add = Some(line_no);
        }
    }
    calls
}

pub(super) fn doc_base_files(files: &FileTree) -> Vec<DocBaseFile> {
    files
        .entries()
        .iter()
        .filter(|entry| is_file(entry) && is_doc_base_child(entry.path()))
        .filter_map(|entry| parse_doc_base_entry(files, entry))
        .collect()
}

fn parse_doc_base_entry(files: &FileTree, entry: &Entry) -> Option<DocBaseFile> {
    let bytes = files.read(entry).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let parsed = parse_doc_base(text);
    Some(DocBaseFile {
        path: entry.path().to_path_buf(),
        no_format: !parsed.has_format,
        abstract_separators: parsed.abstract_separators,
        unknown_fields: parsed.unknown_fields,
    })
}

struct ParsedDocBase {
    has_format: bool,
    abstract_separators: Vec<u32>,
    unknown_fields: Vec<(u32, String)>,
}

const DOCBASE_MAIN_FIELDS: &[&str] = &["Document", "Title", "Section", "Abstract", "Author"];
const DOCBASE_FORMAT_FIELDS: &[&str] = &["Format", "Files", "Index"];

fn parse_doc_base(text: &str) -> ParsedDocBase {
    let mut has_format = false;
    let mut separators = Vec::new();
    let mut unknown_fields = Vec::new();
    let mut format_section = false;
    let mut field: Option<OpenField> = None;
    for (index, line) in physical_lines(text).into_iter().enumerate() {
        let position = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if let Some(caps) = DOC_FIELD.captures(line) {
            if let Some(open) = field.take() {
                finish_field(
                    &open,
                    format_section,
                    &mut has_format,
                    &mut separators,
                    &mut unknown_fields,
                );
            }
            field = Some(OpenField {
                name: caps[1].to_string(),
                last_line: position,
                vals: vec![caps[2].to_string()],
            });
        } else if let Some(open) = field.as_mut()
            && DOC_CONTINUATION.is_match(line)
        {
            open.vals.push(line.to_string());
            open.last_line = position;
        } else if DOC_SEPARATOR.is_match(line)
            && let Some(open) = field.take()
        {
            finish_field(
                &open,
                format_section,
                &mut has_format,
                &mut separators,
                &mut unknown_fields,
            );
            format_section = true;
        }
    }
    if let Some(open) = field {
        finish_field(
            &open,
            format_section,
            &mut has_format,
            &mut separators,
            &mut unknown_fields,
        );
    }
    ParsedDocBase {
        has_format,
        abstract_separators: separators,
        unknown_fields,
    }
}

fn finish_field(
    open: &OpenField,
    format_section: bool,
    has_format: &mut bool,
    separators: &mut Vec<u32>,
    unknown_fields: &mut Vec<(u32, String)>,
) {
    let known = if format_section {
        DOCBASE_FORMAT_FIELDS
    } else {
        DOCBASE_MAIN_FIELDS
    };
    if !known.contains(&open.name.as_str()) {
        unknown_fields.push((open.last_line, open.name.clone()));
    }
    if open.name == "Format" {
        *has_format = true;
    }
    if open.name == "Abstract" {
        abstract_separators(open, separators);
    }
}

/// Lintian's pointer: `last_line - $#vals + idx` for each bad continuation.
fn abstract_separators(open: &OpenField, separators: &mut Vec<u32>) {
    if open.vals.len() < 2 {
        return;
    }
    let last_index = u32::try_from(open.vals.len() - 1).unwrap_or(u32::MAX);
    for (idx, value) in open.vals.iter().enumerate().skip(1) {
        let Some(caps) = ABSTRACT_DOT.captures(value) else {
            continue;
        };
        if &caps[1] != " " || !caps[2].is_empty() {
            let index = u32::try_from(idx).unwrap_or(u32::MAX);
            separators.push(open.last_line - last_index + index);
        }
    }
}

fn file_paths(files: &FileTree, pattern: &Regex) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .path(pattern)
        .into_iter()
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

fn is_menu_readme(path: &Path) -> bool {
    matches!(
        path.to_str(),
        Some("usr/lib/menu/README" | "usr/share/menu/README")
    )
}

fn is_doc_base_child(path: &Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    let Some(name) = text.strip_prefix("usr/share/doc-base/") else {
        return false;
    };
    !name.is_empty() && !name.contains('/')
}

fn is_file(entry: &Entry) -> bool {
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
}

fn control_text(files: &FileTree, name: &str) -> Option<String> {
    let entry = files.get(name)?;
    if !is_file(entry) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Physical lines, without a phantom empty line after a trailing newline.
fn physical_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
}

fn strip_hash_comment(line: &str) -> &str {
    match line.find('#') {
        Some(index) => &line[..index],
        None => line,
    }
}
