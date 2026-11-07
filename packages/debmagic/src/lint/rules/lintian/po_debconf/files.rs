use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{Entry, EntryKind, FileTree};

static TEMPLATES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+\.)?templates(\..+)?$").expect("valid regex"));
static LANG_TEMPLATES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"templates\.\w\w(_\w\w)?$").expect("valid regex"));
static PO_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z]{2,3}(_[A-Z]{2})?(?:@[^.]+)?\.po$").expect("valid regex"));

pub(super) fn is_po_name(name: &str) -> bool {
    PO_NAME.is_match(name)
}

/// Direct children of `debian/` that are debconf templates, not per-language
/// copies (`templates.xx` / `templates.xx_YY`).
pub(super) fn template_files(files: &FileTree) -> Vec<&Entry> {
    let debian = Path::new("debian");
    files
        .entries()
        .iter()
        .filter(|entry| {
            entry.path().parent() == Some(debian)
                && matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                && entry
                    .path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| TEMPLATES.is_match(name) && !LANG_TEMPLATES.is_match(name))
        })
        .collect()
}

/// `debian/po` when it is a directory, otherwise `debian/po.in`.
pub(super) fn po_directory(files: &FileTree) -> Option<&str> {
    if files
        .get("debian/po")
        .is_some_and(|entry| entry.kind() == EntryKind::Directory)
    {
        return Some("debian/po");
    }
    if files
        .get("debian/po.in")
        .is_some_and(|entry| entry.kind() == EntryKind::Directory)
    {
        return Some("debian/po.in");
    }
    None
}

pub(super) fn po_children<'a>(files: &'a FileTree, directory: &str) -> Vec<&'a Entry> {
    let parent = Path::new(directory);
    files
        .entries()
        .iter()
        .filter(|entry| entry.path().parent() == Some(parent))
        .filter(|entry| {
            let Some(name) = entry.path().file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            name.ends_with(".po") || entry.kind() == EntryKind::Directory
        })
        .collect()
}

pub(super) fn read(files: &FileTree, path: &Path) -> Option<Vec<u8>> {
    let entry = files.get(path)?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    files.read(entry).ok()
}

pub(super) fn basename(entry: &Entry) -> Option<&str> {
    entry.path().file_name().and_then(|name| name.to_str())
}

pub(super) fn path_buf(entry: &Entry) -> PathBuf {
    entry.path().to_path_buf()
}
