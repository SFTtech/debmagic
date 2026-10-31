//! Control members the `debian/debconf` Check reads.

use std::path::PathBuf;

use crate::lint::diagnostic::Location;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};

pub(super) fn located(path: PathBuf, line: Option<u32>) -> Location {
    Location {
        path,
        line,
        column: None,
    }
}

pub(super) struct TextFile {
    pub path: PathBuf,
    pub text: String,
}

/// Whether `config`, `templates`, or a debconf-using `preinst` is present.
///
/// Lintian returns before every installable Tag unless one of those is set.
#[derive(Clone, Copy)]
pub(super) struct Gate {
    pub config: bool,
    pub templates: bool,
    pub preinst_debconf: bool,
}

impl Gate {
    pub(super) fn open(&self) -> bool {
        self.config || self.templates || self.preinst_debconf
    }
}

pub(super) fn gate(files: &FileTree) -> Gate {
    let preinst_debconf =
        read_lossy(files, "preinst").is_some_and(|file| preinst_uses_debconf(&file.text));
    Gate {
        config: is_present(files, "config"),
        templates: is_present(files, "templates"),
        preinst_debconf,
    }
}

/// `preinst` loads a confmodule. No shebang is required. Comments are dropped
/// first, the same way lintian strips `#.*` before the search.
pub(super) fn preinst_uses_debconf(text: &str) -> bool {
    text.lines().any(|line| {
        let code = line.split('#').next().unwrap_or("");
        code.contains("/usr/share/debconf/confmodule")
            || code.contains("Debconf::Client::ConfModule")
            || code.contains("Debian::DebConf::Client::ConfModule")
    })
}

fn is_present(files: &FileTree, name: &str) -> bool {
    files.get(name).is_some_and(is_regular)
}

pub(super) fn read_utf8(files: &FileTree, name: &str) -> Option<TextFile> {
    let (path, bytes) = read_bytes(files, name)?;
    let text = String::from_utf8(bytes).ok()?;
    Some(TextFile { path, text })
}

pub(super) fn read_lossy(files: &FileTree, name: &str) -> Option<TextFile> {
    let (path, bytes) = read_bytes(files, name)?;
    Some(TextFile {
        path,
        text: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

fn read_bytes(files: &FileTree, name: &str) -> Option<(PathBuf, Vec<u8>)> {
    let entry = files.get(name)?;
    if !is_regular(entry) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    Some((entry.path().to_path_buf(), bytes))
}

fn is_regular(entry: &Entry) -> bool {
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
}
