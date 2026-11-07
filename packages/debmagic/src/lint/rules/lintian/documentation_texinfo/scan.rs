//! Info documents under `usr/share/info`, as `documentation/texinfo` reads them.
//!
//! `dir`, `dir.old`, and their `.gz` forms are ignored. Directory entries are
//! read only from gzip members whose names are not `*-<digits>.gz`.

use std::io::Read;
use std::path::PathBuf;
use std::sync::LazyLock;

use flate2::read::GzDecoder;
use regex::Regex;

use crate::lint::file_tree::{EntryKind, FileTree};

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
