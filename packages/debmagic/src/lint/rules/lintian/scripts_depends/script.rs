//! Maintainer-script text as lintian's `scripts` and `init-d/maintainer-script` Checks read it.
//!
//! A Control file is a maintainer script when its name is `config…` or ends in
//! `preinst`, `postinst`, `prerm`, or `postrm`, and it is a regular file whose
//! shebang names an interpreter (`env` stripped). Blank lines and full-line
//! comments are dropped. A trailing `\` joins the next code line; a comment or
//! blank line in between does not end that join, and a join that never
//! finishes is not checked. The reported line is the last physical line of the
//! join. Shell continuations are joined only for a shell interpreter
//! (`sh` and the `ash` / `bash` / `dash` / `csh` / `ksh` / `zsh` basenames).
//! `init-d/maintainer-script` joins every script.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{EntryKind, FileTree};

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

/// How a trailing `\` is joined.
pub(super) enum Continuations {
    /// `scripts`: join only when the interpreter is a shell.
    ShellOnly,
    /// `init-d/maintainer-script`: join every script.
    Always,
}

/// One code line of a maintainer script, after comments and continuations.
pub(super) struct ScriptLine {
    pub(super) path: PathBuf,
    /// 1-based line of the last physical line that makes up this code line.
    pub(super) number: u32,
    pub(super) text: String,
    pub(super) shell: bool,
}

/// Code lines of every maintainer script in `files`, in file-tree order.
pub(super) fn maintainer_script_lines(
    files: &FileTree,
    continuations: Continuations,
) -> Vec<ScriptLine> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .path(&SCRIPT_NAME)
        .into_iter()
        .flat_map(|entry| {
            let Ok(bytes) = files.read(entry) else {
                return Vec::new();
            };
            let text = String::from_utf8_lossy(&bytes);
            let Some(interpreter) = interpreter(&text) else {
                return Vec::new();
            };
            let shell = is_shell(&interpreter);
            let join = match continuations {
                Continuations::Always => true,
                Continuations::ShellOnly => shell,
            };
            logical_lines(entry.path(), &text, join, shell)
        })
        .collect()
}

/// lintian's `interpreter`: the first command on the shebang, `env` stripped.
fn interpreter(content: &str) -> Option<String> {
    if content.starts_with("#!#!#!") || !content.starts_with("#!") {
        return None;
    }
    let rest = content[2..].split('\n').next().unwrap_or("");
    let without_env = ENV_PREFIX.replace(rest.trim(), "");
    let token = without_env.split_whitespace().next()?;
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

/// lintian's `is_shell_script`: the interpreter basename is a known shell.
fn is_shell(interpreter: &str) -> bool {
    let base = interpreter.rsplit('/').next().unwrap_or(interpreter);
    SHELL.is_match(base)
}

fn logical_lines(path: &Path, text: &str, join: bool, shell: bool) -> Vec<ScriptLine> {
    let mut lines = Vec::new();
    let mut stashed = String::new();
    for (index, raw) in text.split('\n').enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if BLANK.is_match(raw) || COMMENT_LINE.is_match(raw) {
            continue;
        }
        let without_comment = remove_comments(raw);
        if join && without_comment.ends_with('\\') {
            stashed.push_str(&without_comment[..without_comment.len() - 1]);
            continue;
        }
        let mut combined = std::mem::take(&mut stashed);
        combined.push_str(&without_comment);
        lines.push(ScriptLine {
            path: path.to_path_buf(),
            number,
            text: combined,
            shell,
        });
    }
    lines
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
