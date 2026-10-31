//! Init scripts and whether a systemd unit is shipped, as `systemd`'s
//! `missing-systemd-service-for-init.d-script` and
//! `init.d-script-does-not-source-init-functions` read them.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{Entry, EntryKind, FileTree};

static WHITELIST: LazyLock<Vec<String>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/systemd/init-whitelist"))
        .map(str::to_string)
        .collect()
});

static INIT_D_SCRIPT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A#!\s*(?:/usr/bin/env)?\s*(?:/usr)?/lib/init/init-d-script").expect("valid regex")
});
static LSB_SOURCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\.|source)\s+(?:/usr)?/lib/(?:lsb/init-functions|init/init-d-script)")
        .expect("valid regex")
});
static RCS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#.*Default-Start:.*S").expect("valid regex"));
static SERVICE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/systemd/system/.*\.service$").expect("valid regex"));

#[derive(Debug)]
pub(super) struct InitHit {
    pub path: PathBuf,
    pub service: String,
    pub sources_init: bool,
    pub rc_s: bool,
}

pub(super) fn init_scripts(files: &FileTree, package: &str) -> (bool, Vec<InitHit>) {
    let any_service = files.query().path(&SERVICE).into_iter().next().is_some();
    if package == "initscripts" {
        return (any_service, Vec::new());
    }
    let hits = files
        .entries()
        .iter()
        .filter_map(|entry| init_hit(files, entry))
        .collect();
    (any_service, hits)
}

fn init_hit(files: &FileTree, entry: &Entry) -> Option<InitHit> {
    let path = entry.path().to_str()?;
    let parent = entry.path().parent().and_then(Path::to_str).unwrap_or("");
    if parent != "etc/init.d" || entry.kind() == EntryKind::Directory {
        return None;
    }
    let base = entry.path().file_name().and_then(|name| name.to_str())?;
    if matches!(base, "README" | "skeleton" | "rc" | "rcS") {
        return None;
    }
    if entry.link_target().is_some_and(|target| {
        target == Path::new("lib/init/upstart-job") || target == Path::new("/lib/init/upstart-job")
    }) {
        return None;
    }
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let text = files
        .read(entry)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default();
    let (sources_init, rc_s) = scan(&text);
    let service = base.strip_suffix(".sh").unwrap_or(base).to_string();
    Some(InitHit {
        path: PathBuf::from(path),
        service,
        sources_init,
        rc_s,
    })
}

fn scan(text: &str) -> (bool, bool) {
    let mut sources = false;
    let mut rc_s = false;
    for (index, raw) in text.split('\n').enumerate() {
        let line = raw.trim_start();
        if index == 0 && INIT_D_SCRIPT.is_match(line) {
            sources = true;
        }
        if RCS.is_match(line) {
            rc_s = true;
        }
        if line.starts_with('#') {
            continue;
        }
        if LSB_SOURCE.is_match(line) {
            sources = true;
        }
    }
    (sources, rc_s)
}

pub(super) fn whitelisted(service: &str) -> bool {
    WHITELIST.iter().any(|name| name == service)
}
