//! The printable strings of a file, as `strings --all` prints them, shared by
//! the Rules that read them (lintian's `$item->strings`, filled by
//! `Lintian::Index::Strings` for ELF files outside `usr/lib/debug/`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::lint::elf::entry_bytes;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::file_type::FileType;

/// The printable runs of at least four characters, as `strings(1)` prints
/// them: ASCII graphic characters and tabs. Each run is one element.
pub(crate) fn printable_strings(bytes: &[u8]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut run = String::new();
    for &byte in bytes {
        if byte == b'\t' || (0x20..0x7f).contains(&byte) {
            run.push(char::from(byte));
        } else {
            if run.len() >= 4 {
                strings.push(std::mem::take(&mut run));
            }
            run.clear();
        }
    }
    if run.len() >= 4 {
        strings.push(run);
    }
    strings
}

/// The printable strings of `bytes`, one per line, as `strings` writes them.
#[allow(dead_code)] // seam API; no Rule reads it yet
pub(crate) fn strings_text(bytes: &[u8]) -> String {
    let mut text = String::new();
    for string in printable_strings(bytes) {
        text.push_str(&string);
        text.push('\n');
    }
    text
}

/// `strings` output of the ELF Data files that are regular files or
/// hardlinks, keyed by path, computed once per Subject. lintian skips
/// `usr/lib/debug/`.
#[derive(Default)]
#[allow(dead_code)] // seam API; no Rule reads it yet
pub(crate) struct FileStrings {
    texts: BTreeMap<PathBuf, String>,
}

impl FileStrings {
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub(crate) fn from_tree(files: &FileTree) -> Self {
        let texts = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| !entry.path().starts_with("usr/lib/debug"))
            .filter_map(|entry| {
                let bytes = entry_bytes(files, entry)?;
                if !matches!(FileType::from_bytes(&bytes), Some(FileType::Elf { .. })) {
                    return None;
                }
                Some((entry.path().to_path_buf(), strings_text(&bytes)))
            })
            .collect();
        Self { texts }
    }

    /// The strings of the ELF file at `path`, one per line.
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub(crate) fn get(&self, path: impl AsRef<Path>) -> Option<&str> {
        self.texts.get(path.as_ref()).map(String::as_str)
    }

    /// In path order.
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.texts
            .iter()
            .map(|(path, text)| (path.as_path(), text.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_of_four_printable_bytes_and_tabs() {
        assert_eq!(
            printable_strings(b"\x01abcd\0xy\0hello\tworld\x7f"),
            ["abcd", "hello\tworld"]
        );
        assert!(printable_strings(b"\x7fELF").is_empty());
    }
}
