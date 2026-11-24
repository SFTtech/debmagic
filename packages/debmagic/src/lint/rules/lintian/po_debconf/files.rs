use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{Entry, EntryKind, FileTree};

static TEMPLATES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+\.)?templates(\..+)?$").expect("valid regex"));
static LANG_TEMPLATES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"templates\.\w\w(_\w\w)?$").expect("valid regex"));
static PO_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z]{2,3}(_[A-Z]{2})?(?:@[^.]+)?\.po$").expect("valid regex"));
static TEMPLATE_FIELD: LazyLock<regex::bytes::Regex> =
    LazyLock::new(|| regex::bytes::Regex::new(r"(?i-u)^Template: (\S+)").expect("valid regex"));
static DESCRIPTION_FIELD: LazyLock<regex::bytes::Regex> =
    LazyLock::new(|| regex::bytes::Regex::new(r"(?i-u)^_?Description: (.+)").expect("valid regex"));

/// Templates that are not debconf questions for lintian's `has_template`.
const SHARED_TEMPLATES: [&[u8]; 2] = [b"shared/packages-wordlist", b"shared/packages-ispell"];

pub(super) fn is_po_name(name: &str) -> bool {
    PO_NAME.is_match(name)
}

/// lintian's `$has_template`: some non-language template in `debian/` has a
/// translatable `Description`. Templates named in [`SHARED_TEMPLATES`], under
/// `/languages`, and `for internal use` descriptions do not count. Symlinked
/// templates are not read (lintian reads them).
pub(super) fn has_translatable_template(files: &FileTree) -> bool {
    template_files(files).into_iter().any(|entry| {
        let Ok(bytes) = files.read(entry) else {
            return false;
        };
        let mut in_template = false;
        for line in bytes.split(|&byte| byte == b'\n') {
            if line.starts_with(b"#") {
                continue;
            }
            if let Some(captures) = TEMPLATE_FIELD.captures(line) {
                let name = &captures[1];
                if SHARED_TEMPLATES.contains(&name) || name.ends_with(b"/languages") {
                    continue;
                }
                in_template = true;
            } else if in_template {
                if let Some(captures) = DESCRIPTION_FIELD.captures(line) {
                    if !String::from_utf8_lossy(&captures[1]).contains("for internal use") {
                        return true;
                    }
                } else if line.is_empty() {
                    in_template = false;
                }
            }
        }
        false
    })
}

/// The regular file (or hardlink) at `path`, after following symbolic links
/// inside the tree (lintian's `is_open_ok`).
pub(super) fn open_file<'a>(files: &'a FileTree, path: &Path) -> Option<&'a Entry> {
    resolve(files, path)
        .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
}

/// Entry at `path` after following symbolic links inside the tree (lintian's
/// `resolve_path`). `None` when the path is missing or the links loop.
/// Copy of the resolver in `testsuite/sections.rs`, kept private here.
pub(super) fn resolve<'a>(files: &'a FileTree, path: &Path) -> Option<&'a Entry> {
    let mut pending: VecDeque<OsString> = normal_parts(path).into();
    let mut resolved: Vec<OsString> = Vec::new();
    let mut links = 0;
    while let Some(part) = pending.pop_front() {
        if part == "." {
            continue;
        }
        if part == ".." {
            resolved.pop();
            continue;
        }
        resolved.push(part);
        let current: PathBuf = resolved.iter().collect();
        let Some(target) = files
            .get(&current)
            .and_then(|entry| entry.link_target())
            .map(Path::to_path_buf)
        else {
            continue;
        };
        links += 1;
        if links > 40 {
            return None;
        }
        resolved.pop();
        if target.has_root() {
            resolved.clear();
        }
        for part in normal_parts(&target).into_iter().rev() {
            pending.push_front(part);
        }
    }
    if resolved.is_empty() {
        return None;
    }
    let target: PathBuf = resolved.iter().collect();
    files.get(target)
}

fn normal_parts(path: &Path) -> Vec<OsString> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_os_string()),
            Component::CurDir => Some(OsString::from(".")),
            Component::ParentDir => Some(OsString::from("..")),
            Component::RootDir | Component::Prefix(_) => None,
        })
        .collect()
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
    files.read_file(path)
}

pub(super) fn basename(entry: &Entry) -> Option<&str> {
    entry.path().file_name().and_then(|name| name.to_str())
}

pub(super) fn path_buf(entry: &Entry) -> PathBuf {
    entry.path().to_path_buf()
}
