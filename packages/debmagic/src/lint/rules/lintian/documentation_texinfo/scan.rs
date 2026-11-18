//! Info documents under `usr/share/info`, as `documentation/texinfo` reads them.
//!
//! `dir`, `dir.old`, and their `.gz` forms are ignored. Directory entries are
//! read only from gzip members whose names are not `*-<digits>.gz`.

use std::io::Read;
use std::path::PathBuf;
use std::sync::LazyLock;

use flate2::read::GzDecoder;
use regex::Regex;

use crate::lint::file_tree::{Entry, EntryKind, FileTree};

static NUMBERED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-\d+\.gz").expect("valid regex"));
static DIR_SECTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^INFO-DIR-SECTION\s+\S").expect("valid regex"));
static DIR_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^START-INFO-DIR-ENTRY\b").expect("valid regex"));
static DIR_END: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^END-INFO-DIR-ENTRY\b").expect("valid regex"));

pub(super) struct InfoDocument {
    pub path: PathBuf,
    pub missing_section: bool,
    pub missing_entry: bool,
}

pub(super) fn info_documents(files: &FileTree) -> Vec<InfoDocument> {
    let mut found = Vec::new();
    for entry in files.entries() {
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            continue;
        }
        let Some(path) = entry.path().to_str() else {
            continue;
        };
        if !path.starts_with("usr/share/info/") {
            continue;
        }
        let Some(base) = entry.path().file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if matches!(base, "dir" | "dir.old" | "dir.gz" | "dir.old.gz") {
            continue;
        }
        if NUMBERED.is_match(base) {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        if !bytes.starts_with(&[0x1f, 0x8b]) {
            continue;
        }
        let mut decoder = GzDecoder::new(bytes.as_slice());
        let mut text = String::new();
        if decoder.read_to_string(&mut text).is_err() {
            continue;
        }
        let mut section = false;
        let mut start = false;
        let mut end = false;
        for line in text.lines() {
            if DIR_SECTION.is_match(line) {
                section = true;
            }
            if DIR_START.is_match(line) {
                start = true;
            }
            if DIR_END.is_match(line) {
                end = true;
            }
        }
        found.push(InfoDocument {
            path: entry.path().to_path_buf(),
            missing_section: !section,
            missing_entry: !(start && end),
        });
    }
    found
}

/// Entries `documentation/texinfo` reads as documents: files, hardlinks and
/// symlinks under `usr/share/info`, skipping names that start with `dir`.
pub(super) fn info_items<'a>(files: &'a FileTree) -> impl Iterator<Item = &'a Entry> + 'a {
    files.entries().iter().filter(|entry| {
        matches!(
            entry.kind(),
            EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
        ) && entry.path().starts_with("usr/share/info")
            && !basename(entry).starts_with("dir")
    })
}

/// Final path component, lossily decoded.
pub(super) fn basename(entry: &Entry) -> String {
    entry
        .path()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Perl `split /\./`: trailing empty fields are dropped.
pub(super) fn name_pieces(basename: &str) -> Vec<&str> {
    let mut pieces: Vec<&str> = basename.split('.').collect();
    while pieces.last() == Some(&"") {
        pieces.pop();
    }
    pieces
}

/// `info` or `info-<digits>`.
pub(super) fn is_info_piece(piece: &str) -> bool {
    match piece.strip_prefix("info") {
        Some("") => true,
        Some(rest) => rest
            .strip_prefix('-')
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())),
        None => false,
    }
}

/// Image extensions that `documentation/texinfo` ignores.
pub(super) fn is_image(extension: &str) -> bool {
    matches!(extension, "png" | "jpg" | "jpeg")
}

/// libmagic's `gzip compressed data`: the gzip magic bytes.
pub(super) fn is_gzip(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1f, 0x8b])
}

/// libmagic's `max compression`: gzip data whose header XFL byte is 2.
pub(super) fn is_max_compression(bytes: &[u8]) -> bool {
    is_gzip(bytes) && bytes.get(8) == Some(&2)
}
