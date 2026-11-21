use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{Entry, EntryKind, FileTree};

static WEB_SERVER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^apache2(:?\.2)?(?:-\w+)?$").expect("valid regex"));
static RELATED_MODULE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/apache2/modules/.*\.so$").expect("valid regex"));
static RELATED_CONF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^etc/apache2/(?:conf|site)-(?:available|enabled)/").expect("valid regex")
});
static RELATED_LEGACY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/apache2/conf\.d/").expect("valid regex"));
static SCRIPT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^config|(?:pre|post)(?:inst|rm)$").expect("valid regex"));
static ENV_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/usr/bin/env\s+").expect("valid regex"));
static SHELL_BASENAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[bd]?a|t?c|(?:pd|m)?k|z)?sh$").expect("valid regex"));
static INSPECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:etc/apache2/(?:conf|site)-available/.+|etc/apache2/conf\.d/.+|etc/apache2/mods-available/.+\.(?:load|conf))$",
    )
    .expect("valid regex")
});

pub(super) fn is_web_server(name: &str) -> bool {
    WEB_SERVER.is_match(name)
}

pub(super) fn is_related(files: &FileTree) -> bool {
    files.entries().iter().any(|entry| {
        let Some(path) = entry.path().to_str() else {
            return false;
        };
        RELATED_MODULE.is_match(path)
            || RELATED_CONF.is_match(path)
            || RELATED_LEGACY.is_match(path)
    })
}

pub(super) fn inspect_conf(path: &str) -> bool {
    INSPECT.is_match(path)
}

pub(super) struct ScriptLine {
    pub path: PathBuf,
    pub line: u32,
    pub text: String,
}

/// Physical lines of shell maintainer scripts that name an interpreter.
pub(super) fn shell_lines(files: &FileTree) -> Vec<ScriptLine> {
    let mut lines = Vec::new();
    for entry in files.entries() {
        if entry.kind() != EntryKind::RegularFile && entry.kind() != EntryKind::Hardlink {
            continue;
        }
        let Some(name) = entry.path().file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !SCRIPT_NAME.is_match(name) {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        if !is_shell(&bytes) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        for (index, line) in text.lines().enumerate() {
            lines.push(ScriptLine {
                path: entry.path().to_path_buf(),
                line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                text: line.to_string(),
            });
        }
    }
    lines
}

static MODULE_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/apache2/modules/(.*)\.so$").expect("valid regex"));
static MODULE_STEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^mod.(.*)$").expect("valid regex"));
static WEB_APP_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/apache2/(?:conf|site)-available/").expect("valid regex"));

/// `(path, module)` of each non-directory `usr/lib/apache2/modules/<module>.so`.
pub(super) fn modules(files: &FileTree) -> Vec<(PathBuf, String)> {
    files
        .entries()
        .iter()
        .filter(|entry| entry.kind() != EntryKind::Directory)
        .filter_map(|entry| {
            let path = entry.path().to_str()?;
            let module = MODULE_FILE.captures(path)?.get(1)?.as_str().to_string();
            Some((entry.path().to_path_buf(), module))
        })
        .collect()
}

/// The load and conf file lintian looks up for `module`: `mod_foo` gives
/// `etc/apache2/mods-available/foo.{load,conf}`. A name without the `mod`
/// prefix is looked up unchanged.
pub(super) fn module_files(module: &str) -> (String, String) {
    match MODULE_STEM.captures(module).and_then(|found| found.get(1)) {
        Some(stem) => (
            format!("etc/apache2/mods-available/{}.load", stem.as_str()),
            format!("etc/apache2/mods-available/{}.conf", stem.as_str()),
        ),
        None => (module.to_string(), module.to_string()),
    }
}

/// The files lintian's `inspect_conf_file` reads: non-directory web application
/// files (`conf-available`, `site-available`, `conf.d`), then the load and conf
/// file each module's `.so` finds in the tree. Entries may be links; read them
/// through `open_entry`.
pub(super) fn inspected_conf_files(files: &FileTree) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for entry in files.entries() {
        if entry.kind() == EntryKind::Directory {
            continue;
        }
        let Some(path) = entry.path().to_str() else {
            continue;
        };
        if WEB_APP_FILE.is_match(path) || RELATED_LEGACY.is_match(path) {
            paths.push(entry.path().to_path_buf());
        }
    }
    for (_, module) in modules(files) {
        let (load, conf) = module_files(&module);
        for name in [load, conf] {
            if files.get(&name).is_some() {
                paths.push(PathBuf::from(name));
            }
        }
    }
    paths
}

/// The regular file or hardlink at `path`, following relative symlinks that
/// stay inside the tree (lintian's `is_open_ok`). `None` for directories,
/// missing, dangling, absolute or escaping links.
pub(super) fn open_entry<'t>(files: &'t FileTree, path: &Path) -> Option<&'t Entry> {
    let mut current = files.get(path)?;
    for _ in 0..8 {
        match current.kind() {
            EntryKind::RegularFile | EntryKind::Hardlink => return Some(current),
            EntryKind::Directory => return None,
            EntryKind::Symlink => {
                let mut resolved = current.path().parent()?.to_path_buf();
                for component in current.link_target()?.components() {
                    match component {
                        Component::Normal(part) => resolved.push(part),
                        Component::CurDir => {}
                        Component::ParentDir => {
                            if !resolved.pop() {
                                return None;
                            }
                        }
                        Component::RootDir | Component::Prefix(_) => return None,
                    }
                }
                current = files.get(&resolved)?;
            }
        }
    }
    None
}

fn is_shell(content: &[u8]) -> bool {
    if content.starts_with(b"#!#!#!") || !content.starts_with(b"#!") {
        return false;
    }
    let rest = &content[2..];
    let line = rest.split(|&byte| byte == b'\n').next().unwrap_or_default();
    let line = String::from_utf8_lossy(line);
    let line = line.trim();
    if line.split_whitespace().next().is_none() {
        return false;
    }
    let stripped = ENV_PREFIX.replace(line, "");
    let Some(first) = stripped.split_whitespace().next() else {
        return false;
    };
    let base = first.rsplit('/').next().unwrap_or(first);
    SHELL_BASENAME.is_match(base)
}
