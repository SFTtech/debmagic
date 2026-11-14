use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{EntryKind, FileTree};

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
