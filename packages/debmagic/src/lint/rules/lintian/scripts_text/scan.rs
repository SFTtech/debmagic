//! Maintainer-script text as lintian's `scripts` Check reads Control files.
//!
//! A maintainer script is a regular Control file whose name is `config…` or
//! ends in `preinst`, `postinst`, `prerm`, or `postrm`. Command Tags scan a
//! file only when the shebang names an interpreter. Shell-only Tags use
//! lintian's `is_shell_script` basename (`sh` and the `bash` / `dash` / `ksh`
//! / `zsh` family), with a leading `/usr/bin/env` stripped. Other interpreters
//! are still scanned: this does not consult lintian's interpreter table.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{Entry, EntryKind, FileTree};

static SCRIPT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^config|(?:pre|post)(?:inst|rm)$").expect("valid regex"));
static ENV_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/usr/bin/env\s+").expect("valid regex"));
static SHELL_BASENAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[bd]?a|t?c|(?:pd|m)?k|z)?sh$").expect("valid regex"));
static BLANK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*$").expect("valid regex"));
static COMMENT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("valid regex"));
static SINGLE_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\](?:\\\\)*)'(?:\\.|[^\\'])+'"#).expect("valid regex"));
static DOUBLE_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\](?:\\\\)*)"(?:\\.|[^\\"])+""#).expect("valid regex"));
static TRAILING_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[^\[\\])[\s&;()](#.*$)").expect("valid regex"));
static AUTO_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^# Automatically added by \S+\s*$").expect("valid regex"));

const AUTO_END: &str = "# End automatically added section";

/// One logical line of a maintainer script that has a shebang interpreter.
pub(super) struct ScriptLine {
    pub path: PathBuf,
    /// Physical line where the logical line ends. Continuations point here.
    pub line: u32,
    pub text: String,
    pub shell: bool,
    /// Inside `# Automatically added by …` … `# End automatically added section`.
    pub automatic: bool,
}

/// A pattern hit. `message` is empty or the match wrapped in double quotes.
pub(super) struct Finding {
    pub path: PathBuf,
    pub line: u32,
    pub message: String,
}

/// How one bad-command pattern applies to [`script_lines`].
pub(super) struct CommandPattern {
    pub shell_only: bool,
    pub skip_automatic: bool,
    /// Quote the match, with both ends trimmed, the way `pointed_hint` does.
    pub quote: bool,
}

/// Regular maintainer scripts and their bytes.
pub(super) fn read_scripts(files: &FileTree) -> Vec<(PathBuf, Vec<u8>)> {
    maintainer_scripts(files)
        .into_iter()
        .map(|(entry, bytes)| (entry.path().to_path_buf(), bytes))
        .collect()
}

/// Logical lines of maintainer scripts that name an interpreter.
pub(super) fn script_lines(files: &FileTree) -> Vec<ScriptLine> {
    let mut lines = Vec::new();
    for (entry, bytes) in maintainer_scripts(files) {
        let Some(shebang) = shebang(&bytes) else {
            continue;
        };
        let shell = is_shell(&shebang);
        let text = String::from_utf8_lossy(&bytes);
        lines.extend(logical_lines(entry.path(), &text, shell));
    }
    lines
}

pub(super) fn find_commands(
    files: &FileTree,
    pattern: &Regex,
    kind: &CommandPattern,
) -> Vec<Finding> {
    script_lines(files)
        .into_iter()
        .filter(|line| !kind.shell_only || line.shell)
        .filter(|line| !(kind.skip_automatic && line.automatic))
        .filter_map(|line| {
            let found = pattern.find(&line.text)?;
            let message = if kind.quote {
                format!("\"{}\"", found.as_str().trim())
            } else {
                String::new()
            };
            Some(Finding {
                path: line.path,
                line: line.line,
                message,
            })
        })
        .collect()
}

fn maintainer_scripts(files: &FileTree) -> Vec<(&Entry, Vec<u8>)> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .path(&SCRIPT_NAME)
        .into_iter()
        .filter_map(|entry| Some((entry, files.read(entry).ok()?)))
        .collect()
}

/// The shebang line after `#!`, trimmed. `#!#!#!` and an empty interpreter are
/// absent. The line still includes options and a trailing `env` command.
fn shebang(content: &[u8]) -> Option<String> {
    if content.starts_with(b"#!#!#!") || !content.starts_with(b"#!") {
        return None;
    }
    let rest = &content[2..];
    if rest.is_empty() {
        return None;
    }
    let line = rest.split(|&byte| byte == b'\n').next().unwrap_or_default();
    let line = String::from_utf8_lossy(line);
    let line = line.trim();
    line.split_whitespace().next()?;
    Some(line.to_string())
}

fn is_shell(shebang: &str) -> bool {
    let stripped = ENV_PREFIX.replace(shebang, "");
    let Some(first) = stripped.split_whitespace().next() else {
        return false;
    };
    let base = first.rsplit('/').next().unwrap_or(first);
    SHELL_BASENAME.is_match(base)
}

/// Blank lines and comments dropped. In a shell script a trailing `\` joins
/// the next physical line and the hit points at that later line. A `#`
/// comment is removed first, so `\` before a comment does not continue.
fn logical_lines(path: &Path, text: &str, shell: bool) -> Vec<ScriptLine> {
    let mut lines = Vec::new();
    let mut stashed = String::new();
    let mut automatic = false;
    for (index, raw) in text.split('\n').enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if AUTO_START.is_match(raw) {
            automatic = true;
        }
        if raw.trim_end_matches('\r') == AUTO_END {
            automatic = false;
        }
        if BLANK.is_match(raw) || COMMENT_LINE.is_match(raw) {
            continue;
        }
        let without_comment = remove_comments(raw);
        if shell && let Some(continued) = without_comment.strip_suffix('\\') {
            stashed.push_str(continued);
            continue;
        }
        let mut joined = std::mem::take(&mut stashed);
        joined.push_str(&without_comment);
        lines.push(ScriptLine {
            path: path.to_path_buf(),
            line: number,
            text: joined,
            shell,
            automatic,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_follows_the_basename_after_env() {
        assert!(is_shell("/bin/sh"));
        assert!(is_shell("/bin/bash"));
        assert!(is_shell("/bin/dash"));
        assert!(is_shell("/usr/bin/zsh"));
        assert!(is_shell("/bin/ksh"));
        assert!(is_shell("/usr/bin/env sh"));
        assert!(is_shell("/usr/bin/env bash -e"));
        assert!(!is_shell("/usr/bin/perl"));
        assert!(!is_shell("/usr/bin/python"));
        assert!(!is_shell("/usr/bin/env"));
        assert!(!is_shell("/usr/bin/env -S sh"));
    }

    #[test]
    fn shebang_requires_an_interpreter() {
        assert!(shebang(b"#!/bin/sh\n").is_some());
        assert!(shebang(b"#! /bin/sh -e\n").is_some());
        assert!(shebang(b"#!/usr/bin/env sh\n").is_some());
        assert!(shebang(b"#!\n").is_none());
        assert!(shebang(b"#!").is_none());
        assert!(shebang(b"#!   \n").is_none());
        assert!(shebang(b"#!#!#!\n").is_none());
        assert!(shebang(b"install-sgmlcatalog\n").is_none());
    }

    #[test]
    fn continuation_points_at_the_last_physical_line() {
        let text = "#!/bin/sh\ninstall-info foo \\\n    /usr/share/info/bar.info\n";
        let lines = logical_lines(Path::new("postinst"), text, true);
        let hit = lines
            .iter()
            .find(|line| line.text.contains("install-info"))
            .expect("joined command");
        assert_eq!(hit.line, 3);
        assert!(hit.text.contains("/usr/share/info/bar.info"));
        assert!(!hit.text.contains('\\'));
    }

    #[test]
    fn comment_after_a_backslash_ends_the_command() {
        let text = "#!/bin/sh\ninstall-info /usr/share/info/foo \\\n\t--quiet \\ # make it so\n\t--section foo\n";
        let lines = logical_lines(Path::new("postinst"), text, true);
        let hit = lines
            .iter()
            .find(|line| line.text.contains("install-info"))
            .expect("command");
        assert_eq!(hit.line, 3);
        assert!(!hit.text.contains("section"));
        assert!(lines.iter().any(|line| line.line == 4));
    }

    #[test]
    fn non_shell_does_not_join_continuations() {
        let text = "#!/usr/bin/perl\ninstall-info foo \\\nbar\n";
        let lines = logical_lines(Path::new("postinst"), text, false);
        assert!(
            lines
                .iter()
                .any(|line| line.line == 2 && line.text.contains('\\'))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.line == 3 && line.text == "bar")
        );
    }

    #[test]
    fn automatic_section_covers_only_its_body() {
        let text = "\
#!/bin/sh
dpkg-maintscript-helper before
# Automatically added by dh_dummy/12
dpkg-maintscript-helper inside
# End automatically added section
dpkg-maintscript-helper after
";
        let lines = logical_lines(Path::new("postinst"), text, true);
        let flagged: Vec<_> = lines
            .iter()
            .filter(|line| line.text.contains("dpkg-maintscript-helper"))
            .map(|line| (line.line, line.automatic))
            .collect();
        assert_eq!(flagged, vec![(2, false), (4, true), (6, false)]);
    }

    #[test]
    fn comments_are_removed_outside_quotes() {
        assert_eq!(remove_comments("set -e # fail early"), "set -e ");
        assert_eq!(remove_comments("echo \"a # b\""), "echo \"a # b\"");
        assert_eq!(remove_comments("echo 'a # b' # c"), "echo 'a # b' ");
    }
}
