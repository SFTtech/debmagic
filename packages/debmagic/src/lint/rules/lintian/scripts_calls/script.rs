//! Maintainer scripts as lintian's `scripts` Check reads them.
//!
//! Control files named `config…` or ending in `preinst`, `postinst`, `prerm`,
//! or `postrm`, regular files with a shebang. Comments and blank lines are
//! skipped. A shell `\` continuation is joined onto the next code line; lintian
//! deletes the first `\` on the continued line, which is the trailing one only
//! when the line has no earlier backslash. The reported line is that last
//! physical line.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::debian_control::ControlParagraph;
use crate::lint::file_tree::{EntryKind, FileTree};

pub(super) struct LogicalLine {
    pub path: PathBuf,
    pub line: u32,
    pub text: String,
    pub shell: bool,
    /// File name (`postinst`, `postrm`, …).
    pub name: String,
}

pub(super) struct CommandHit {
    pub path: PathBuf,
    pub line: u32,
    pub command: String,
}

static SCRIPT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^config|(?:pre|post)(?:inst|rm)$").expect("valid regex"));
static ENV_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/usr/bin/env\s+").expect("valid regex"));
static SHELL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[bd]?a|t?c|(?:pd|m)?k|z)?sh$").expect("valid regex"));
static BLANK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*$").expect("valid regex"));
static COMMENT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("valid regex"));
static SINGLE_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\](?:\\\\)*)'(?:\\.|[^\\'])+'"#).expect("valid regex"));
static DOUBLE_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\](?:\\\\)*)"(?:\\.|[^\\"])+""#).expect("valid regex"));
static TRAILING_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[^\[\\])[\s&;()](#.*$)").expect("valid regex"));

/// `|| true` on the line, lintian's exemption for a probed command.
pub(super) static OR_TRUE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\|\|\s*true\b").expect("valid regex"));

const STRONG_FIELDS: [&str; 2] = ["Pre-Depends", "Depends"];

/// `Depends` or `Pre-Depends` names one of `packages`.
///
/// Presence of the package name, including as an alternative and with a
/// version or architecture restriction. This is not lintian's `satisfies`.
pub(super) fn strong_relation_mentions(fields: &ControlParagraph, packages: &[&str]) -> bool {
    STRONG_FIELDS.iter().any(|field| {
        fields.relation(field).is_some_and(|relation| {
            relation.alternatives().any(|alternative| {
                packages
                    .iter()
                    .any(|package| alternative.package() == *package)
            })
        })
    })
}

/// Calls of `call` that are not a probe and not in `postrm`.
pub(super) fn prerequisite_hits(
    files: &FileTree,
    call: &Regex,
    dash_x: &Regex,
    which: &Regex,
    command: &Regex,
) -> Vec<CommandHit> {
    maintainer_script_lines(files)
        .into_iter()
        .filter(|line| line.name != "postrm")
        .filter_map(|line| {
            if dash_x.is_match(&line.text)
                || which.is_match(&line.text)
                || command.is_match(&line.text)
                || OR_TRUE.is_match(&line.text)
            {
                return None;
            }
            let command = call.find(&line.text)?.as_str().trim().to_string();
            Some(CommandHit {
                path: line.path,
                line: line.line,
                command,
            })
        })
        .collect()
}

pub(super) fn maintainer_script_lines(files: &FileTree) -> Vec<LogicalLine> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .path(&SCRIPT_NAME)
        .into_iter()
        .filter_map(|entry| {
            let content = files.read(entry).ok()?;
            script_lines(entry.path(), &content)
        })
        .flatten()
        .collect()
}

fn script_lines(path: &Path, content: &[u8]) -> Option<Vec<LogicalLine>> {
    if content.starts_with(b"\x7fELF") {
        return None;
    }
    let basename = interpreter_basename(content)?;
    let shell = SHELL.is_match(&basename);
    let name = path.file_name()?.to_str()?.to_string();
    let text = String::from_utf8_lossy(content);
    let mut lines = Vec::new();
    let mut stashed = String::new();
    for (index, raw) in text.split('\n').enumerate() {
        let line_no = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if BLANK.is_match(raw) || COMMENT_LINE.is_match(raw) {
            continue;
        }
        let without_comment = remove_comments(raw);
        if shell && let Some(continued) = stash_continuation(&without_comment) {
            stashed.push_str(&continued);
            continue;
        }
        let text = std::mem::take(&mut stashed) + &without_comment;
        lines.push(LogicalLine {
            path: path.to_path_buf(),
            line: line_no,
            text,
            shell,
            name: name.clone(),
        });
    }
    Some(lines)
}

/// Shebang command with `/usr/bin/env` and options removed. `None` without an
/// interpreter, including lincity's `#!#!#!` prefix.
fn interpreter_basename(content: &[u8]) -> Option<String> {
    if content.starts_with(b"#!#!#!") || !content.starts_with(b"#!") {
        return None;
    }
    let rest = content[2..]
        .split(|&byte| byte == b'\n')
        .next()
        .unwrap_or_default();
    let line = String::from_utf8_lossy(rest);
    let line = ENV_PREFIX.replace(line.trim(), "");
    let token = line.split_whitespace().next()?;
    let base = token.rsplit('/').next().unwrap_or(token);
    if base.is_empty() {
        None
    } else {
        Some(base.to_string())
    }
}

/// A continued shell line. `None` when `line` does not end in `\`.
///
/// lintian removes the first `\` (`s/\\//`), then appends the following code
/// line.
fn stash_continuation(line: &str) -> Option<String> {
    if !line.ends_with('\\') {
        return None;
    }
    let mut owned = line.to_string();
    if let Some(index) = owned.find('\\') {
        owned.remove(index);
    }
    Some(owned)
}

/// Drop a trailing `# comment`, ignoring `#` inside quoted strings.
fn remove_comments(line: &str) -> String {
    if line.is_empty() {
        return String::new();
    }
    let simplified = SINGLE_QUOTED.replace_all(line, "${1}''");
    let simplified = DOUBLE_QUOTED.replace_all(&simplified, "${1}\"\"");
    match TRAILING_COMMENT
        .captures(&simplified)
        .and_then(|captures| captures.get(1))
    {
        Some(comment) => line.replacen(comment.as_str(), "", 1),
        None => line.to_string(),
    }
}
