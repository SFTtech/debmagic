//! Shared walk of maintainer-script Control files for the `scripts` shell Tags.
//!
//! Lintian scans these in `Lintian::Check::Scripts::visit_control_files` after
//! it has accepted the interpreter. Installed files are not scanned. The
//! interpreters table (paths and dependencies) is not used here: a body is
//! scanned when `Scripts.pm` would not return early for an unknown control
//! interpreter without that table, plus direct shell shebangs lintian 2.139
//! lists as known (`dash`, `ash`, …). `pdksh` matches the shell-basename
//! regex and is not in that list, so it is not scanned. `#!/usr/bin/env sh`
//! on `postinst` / `preinst` / `prerm` is not scanned (`env` is not a known
//! interpreter). `config` and `postrm` are scanned for any interpreter, as
//! are `/usr/local/…` shebangs.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{EntryKind, FileTree};

/// `^config|(?:pre|post)(?:inst|rm)$` — lintian's maintainer-script names.
static SCRIPT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^config|(?:pre|post)(?:inst|rm)$").expect("valid regex"));

/// `/^(?:[bd]?a|t?c|(?:pd|m)?k|z)?sh$/` — lintian's `is_shell_script` basename.
static SHELL_BASENAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[bd]?a|t?c|(?:pd|m)?k|z)?sh$").expect("valid regex"));

/// Command position, lintian's `$LEADING_PATTERN` (2.139, no `errexit` extension).
static LEADING: &str = r#"(?:(?:^|[`&;(|{])\s*|(?:if|then|do|while|!)\s+|env(?:\s+[[:alnum:]_]+=(?:\S+|"[^"]*"|'[^']*'))*\s+)"#;

static SET_E: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"{LEADING}set\s*(?:\s+-(?:-.*|[^e]+))*\s-\w*e")).expect("valid regex")
});

static READ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*read(?:\s|\z)").expect("valid regex"));

static AUTO_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^# Automatically added by \S+\s*$").expect("valid regex"));

static BACKTICK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`]+`").expect("valid regex"));

static BACKTICK_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"{LEADING}(/(?:usr/)?s?bin/[\w.+-]+)(?:\s|;|\z)")).expect("valid regex")
});

static PLAIN_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"{LEADING}(/(?:usr/)?s?bin/[\w.+-]+)(?:\s|;|$)")).expect("valid regex")
});

static TEST_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\s+(?:!\s+)?-x\s+(/(?:usr/)?s?bin/[\w.+-]+)\s+\]").expect("valid regex")
});

static SINGLE_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\](?:\\\\)*)'(?:\\.|[^\\'])+'"#).expect("valid regex"));
static DOUBLE_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\](?:\\\\)*)"(?:\\.|[^\\"])+""#).expect("valid regex"));
static TRAILING_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[^\[\\])[\s&;()](#.*$)").expect("valid regex"));

const DEBCONF: &str = "/usr/share/debconf/confmodule";
const AUTO_END: &str = "# End automatically added section";

/// One logical line of a maintainer script (continuations already joined).
pub(super) struct ScriptLine {
    pub number: u32,
    pub text: String,
    pub in_automatic_section: bool,
}

/// A maintainer script whose body lintian would scan for these Tags.
pub(super) struct ScannedScript {
    pub path: PathBuf,
    pub is_shell: bool,
    /// `set` with an `e` flag was seen. `set -o errexit` does not count (2.139).
    pub saw_sete: bool,
    /// Shebang names the interpreter with an `e` flag (`#!/bin/sh -e`).
    pub saw_bange: bool,
    pub lines: Vec<ScriptLine>,
}

pub(super) fn maintainer_scripts(files: &FileTree) -> Vec<ScannedScript> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .path(&SCRIPT_NAME)
        .into_iter()
        .filter_map(|entry| scan_entry(entry.path(), &files.read(entry).ok()?))
        .collect()
}

fn scan_entry(path: &Path, bytes: &[u8]) -> Option<ScannedScript> {
    let text = String::from_utf8_lossy(bytes);
    let hashbang = hashbang(&text)?;
    let first = first_word(hashbang);
    if !scans_body(path, first) {
        return None;
    }
    let is_shell = is_shell_script(hashbang);
    let bang_base = basename(first);
    let mut saw_bange = false;
    let mut saw_sete = false;
    let mut in_automatic_section = false;
    let mut pending = String::new();
    let mut lines = Vec::new();

    for (index, raw) in physical_lines(&text).into_iter().enumerate() {
        let number = (index + 1) as u32;
        if number == 1 && is_shell && shebang_enables_errexit(raw, bang_base) {
            saw_bange = true;
        }
        if AUTO_START.is_match(raw) {
            in_automatic_section = true;
        }
        // Compared to the raw line, which still has its newline, so a normal
        // end marker does not clear the section.
        if raw == AUTO_END {
            in_automatic_section = false;
        }
        if raw.trim().is_empty() || comment_line(raw) {
            continue;
        }
        let stripped = remove_comments(raw);
        if is_shell && continues(stripped.as_str()) {
            pending.push_str(&chomp(&strip_first_backslash(&stripped)));
            continue;
        }
        let text = std::mem::take(&mut pending) + &chomp(&stripped);
        if is_shell && SET_E.is_match(&text) {
            saw_sete = true;
        }
        lines.push(ScriptLine {
            number,
            text,
            in_automatic_section,
        });
    }

    Some(ScannedScript {
        path: path.to_path_buf(),
        is_shell,
        saw_sete,
        saw_bange,
        lines,
    })
}

/// `read` at the start of a logical line.
pub(super) fn line_is_read(line: &str) -> bool {
    READ.is_match(line)
}

pub(super) fn line_mentions_debconf(line: &str) -> bool {
    line.contains(DEBCONF)
}

/// Whole match of the recursive-privilege pattern, ends trimmed.
pub(super) fn recursive_privilege(line: &str) -> Option<String> {
    let mut index = 0;
    while index < line.len() {
        if line.is_char_boundary(index) && word_boundary(line, index) {
            if let Some(end) = match_chmod_chown(line, index) {
                return Some(trim_ws(&line[index..end]).to_string());
            }
            if let Some(end) = match_find(line, index) {
                return Some(trim_ws(&line[index..end]).to_string());
            }
        }
        index += line[index..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
    }
    None
}

/// Whole match of `rm` of a device node, ends trimmed. Shell lines only.
pub(super) fn removes_device(line: &str) -> Option<String> {
    let rest = strip_rm(line)?;
    let consumed = consume_device(rest)?;
    let prefix = line.len() - rest.len();
    let matched = trim_ws(&line[..prefix + consumed]);
    if matched.is_empty() {
        None
    } else {
        Some(matched.to_string())
    }
}

/// Path hits for `command-with-path-in-maintainer-script`, in lintian's order:
/// every backtick, then test syntax, then the plain command.
pub(super) fn command_paths(line: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for caps in BACKTICK.captures_iter(line) {
        let inner = &caps[0][1..caps[0].len() - 1];
        if let Some(path) = BACKTICK_PATH.captures(inner).and_then(|c| c.get(1)) {
            hits.push(format!("{} (in backticks)", path.as_str()));
        }
    }
    if let Some(path) = TEST_PATH.captures(line).and_then(|c| c.get(1)) {
        hits.push(format!("{} (in test syntax)", path.as_str()));
    }
    let plain = BACKTICK.replace_all(line, "");
    if let Some(path) = PLAIN_PATH.captures(&plain).and_then(|c| c.get(1)) {
        hits.push(format!("{} (plain script)", path.as_str()));
    }
    hits
}

fn scans_body(path: &Path, first_word: &str) -> bool {
    if first_word.is_empty() {
        return false;
    }
    if first_word.starts_with("/usr/local/") {
        return true;
    }
    let base = basename(first_word);
    if matches!(base, "sh" | "bash" | "perl") || scanned_shell(base) {
        return true;
    }
    path.as_os_str() == "config" || path.as_os_str() == "postrm"
}

/// Shell basenames other than the hardcoded `sh` / `bash`. `pdksh` is a shell
/// basename but not a known control interpreter in lintian 2.139.
fn scanned_shell(base: &str) -> bool {
    base != "pdksh" && SHELL_BASENAME.is_match(base)
}

fn hashbang(content: &str) -> Option<&str> {
    if content.starts_with("#!#!#!") || !content.starts_with("#!") {
        return None;
    }
    let line = content[2..].split('\n').next().unwrap_or("");
    Some(trim_ws(line))
}

fn is_shell_script(hashbang: &str) -> bool {
    let rest = match hashbang.strip_prefix("/usr/bin/env") {
        Some(after) if after.starts_with(char::is_whitespace) => trim_ws_start(after),
        _ => hashbang,
    };
    SHELL_BASENAME.is_match(basename(first_word(rest)))
}

fn shebang_enables_errexit(raw: &str, basename: &str) -> bool {
    let pattern = format!(r"/{}\s*.*\s-\w*e\w*\b", regex::escape(basename));
    Regex::new(&pattern).expect("valid regex").is_match(raw)
}

fn first_word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or("")
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn physical_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            lines.push(&text[start..=index]);
            start = index + 1;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

fn comment_line(raw: &str) -> bool {
    let body = raw.strip_suffix('\n').unwrap_or(raw);
    body.trim_start().starts_with('#')
}

/// `/\\$/` — a trailing newline does not count, a CR before it does.
fn continues(line: &str) -> bool {
    let body = line.strip_suffix('\n').unwrap_or(line);
    body.ends_with('\\')
}

fn strip_first_backslash(line: &str) -> String {
    line.replacen('\\', "", 1)
}

fn chomp(line: &str) -> String {
    line.strip_suffix('\n').unwrap_or(line).to_string()
}

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

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn word_boundary(line: &str, index: usize) -> bool {
    let prev = if index == 0 {
        None
    } else {
        line[..index].chars().next_back()
    };
    let next = line[index..].chars().next();
    matches!(
        (prev.map(is_word), next.map(is_word)),
        (Some(true), Some(false) | None) | (Some(false) | None, Some(true))
    )
}

fn match_chmod_chown(line: &str, start: usize) -> Option<usize> {
    let rest = &line[start..];
    // `chmod` and `chown` are the same length.
    if !rest.starts_with("chmod") && !rest.starts_with("chown") {
        return None;
    }
    let cmd = 5;
    let mut best = None;
    let mut index = start + cmd;
    while index < line.len() {
        if line.is_char_boundary(index) {
            let flag = if line[index..].starts_with("--recursive") {
                Some("--recursive".len())
            } else if line[index..].starts_with("-R") {
                Some(2)
            } else {
                None
            };
            if let Some(len) = flag {
                let end = index + len;
                if word_boundary(line, end) {
                    best = Some(end);
                }
            }
        }
        index += line[index..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
    }
    best
}

fn match_find(line: &str, start: usize) -> Option<usize> {
    if !line[start..].starts_with("find") {
        return None;
    }
    let after = start + 4;
    let barrier = first_maxdepth_one(line, after);
    let mut best = None;
    let mut index = after;
    while index < line.len() {
        if line.is_char_boundary(index)
            && (line[index..].starts_with("chmod") || line[index..].starts_with("chown"))
        {
            let end = index + 5;
            if word_boundary(line, end) && exec_before(line, after, index, barrier) {
                best = Some(end);
            }
        }
        index += line[index..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
    }
    best
}

fn exec_before(line: &str, from: usize, cmd_at: usize, barrier: usize) -> bool {
    let limit = cmd_at.min(barrier);
    let mut search = from;
    while search < limit {
        let Some(rel) = line[search..limit].find("exec") else {
            return false;
        };
        let at = search + rel;
        if at < barrier && at + 4 <= cmd_at {
            return true;
        }
        search = at + 1;
    }
    false
}

fn first_maxdepth_one(line: &str, from: usize) -> usize {
    let mut search = from;
    while let Some(rel) = line[search..].find("-maxdepth") {
        let at = search + rel;
        let rest = &line[at + "-maxdepth".len()..];
        let ws: usize = rest
            .chars()
            .take_while(|c| c.is_whitespace())
            .map(char::len_utf8)
            .sum();
        if ws > 0 && rest[ws..].starts_with('1') {
            return at;
        }
        search = at + "-maxdepth".len();
    }
    line.len()
}

fn strip_rm(line: &str) -> Option<&str> {
    let after_ws = trim_ws_start(line);
    let after_rm = after_ws.strip_prefix("rm")?;
    if !after_rm.starts_with(char::is_whitespace) {
        return None;
    }
    Some(trim_ws_start(after_rm))
}

fn consume_device(after_rm: &str) -> Option<usize> {
    let mut ends = Vec::new();
    let mut index = 0;
    for ch in after_rm.chars() {
        if ch == '>' {
            break;
        }
        index += ch.len_utf8();
        if ch.is_whitespace() {
            ends.push(index);
        }
    }
    ends.reverse();
    ends.push(0);
    for end in ends {
        if let Some(dev) = device_path_len(&after_rm[end..]) {
            return Some(end + dev);
        }
    }
    None
}

/// Length of `/dev/…` when the node is not `shm/…` or a dotfile.
fn device_path_len(text: &str) -> Option<usize> {
    let rest = text.strip_prefix("/dev/")?;
    if rest.starts_with("shm/") {
        return None;
    }
    if let Some(after_dot) = rest.strip_prefix('.')
        && after_dot.chars().next().is_some_and(|c| c != '.')
    {
        return None;
    }
    let len: usize = rest
        .chars()
        .take_while(|c| *c != '/' && *c != ' ')
        .map(char::len_utf8)
        .sum();
    if len == 0 {
        None
    } else {
        Some("/dev/".len() + len)
    }
}

fn trim_ws(text: &str) -> &str {
    text.trim_matches(char::is_whitespace)
}

fn trim_ws_start(text: &str) -> &str {
    text.trim_start_matches(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_matches_follow_the_perl_pattern() {
        let cases = [
            ("rm /dev/null", Some("rm /dev/null")),
            ("rm /dev/shm/test", None),
            ("rm /dev/.hiddenfile", None),
            ("rm /tmp/foo /dev/device", Some("rm /tmp/foo /dev/device")),
            ("rm\t/dev/device1", Some("rm\t/dev/device1")),
            ("rm /tmp/foo > /dev/null", None),
            (
                "rm /dev/shm/foo /dev/null",
                Some("rm /dev/shm/foo /dev/null"),
            ),
            ("  rm /dev/null", Some("rm /dev/null")),
            ("rm /dev/null extra", Some("rm /dev/null")),
            ("rm /dev/foo/bar", Some("rm /dev/foo")),
            ("echo rm /dev/null", None),
            ("rm /dev/.", Some("rm /dev/.")),
            ("rm /dev/..foo", Some("rm /dev/..foo")),
            ("rm /dev/.a.b", None),
            ("rm /dev/shm", Some("rm /dev/shm")),
            ("rm -f /dev/null", Some("rm -f /dev/null")),
            ("sudo rm /dev/null", None),
        ];
        for (line, expected) in cases {
            assert_eq!(removes_device(line).as_deref(), expected, "{line}");
        }
    }

    #[test]
    fn recursive_matches_follow_the_perl_pattern() {
        let cases = [
            (
                "chown -R root.root /usr/share/doc/maintainer-scripts",
                Some("chown -R"),
            ),
            ("chown root:root -R /bad", Some("chown root:root -R")),
            (
                "chown root:root --recursive /bad",
                Some("chown root:root --recursive"),
            ),
            (
                "chown --recursive root:root /bad",
                Some("chown --recursive"),
            ),
            ("chmod -R 777 /bad", Some("chmod -R")),
            ("chmod 777 -R /bad", Some("chmod 777 -R")),
            ("chmod 777 --recursive /bad", Some("chmod 777 --recursive")),
            ("chmod --recursive 777 /bad", Some("chmod --recursive")),
            (
                "find /bad -maxdepth 2 -type d -exec chown root:root {} \\;",
                Some("find /bad -maxdepth 2 -type d -exec chown"),
            ),
            (
                "find /bad -maxdepth 2 -type d -exec chmod 777",
                Some("find /bad -maxdepth 2 -type d -exec chmod"),
            ),
            (
                "find /bad -maxdepth 1 -type d -exec chown root:root {} \\;",
                None,
            ),
            ("find /bad -maxdepth  1 -exec chmod", None),
            ("find /ok -exec chown", Some("find /ok -exec chown")),
            ("chown root:root /good", None),
            ("chmod 777 /good", None),
            (
                "find /bad -maxdepth 2 -exec echo; chown -R /x",
                Some("find /bad -maxdepth 2 -exec echo; chown"),
            ),
            ("chmod --recursive -R /bad", Some("chmod --recursive -R")),
            ("  chown -R /tmp", Some("chown -R")),
            (
                "find /bad -maxdepth 1 -type d -exec echo; find /ok -exec chown",
                Some("find /ok -exec chown"),
            ),
            ("chowning -R /tmp", Some("chowning -R")),
            ("find exec mychown", Some("find exec mychown")),
            (
                "find /ok -exec echo -maxdepth 1 chown",
                Some("find /ok -exec echo -maxdepth 1 chown"),
            ),
            ("chown -R; find -maxdepth 1 -exec chmod", Some("chown -R")),
            (
                "prefix find /ok -exec chmod 755 /ok",
                Some("find /ok -exec chmod"),
            ),
        ];
        for (line, expected) in cases {
            assert_eq!(recursive_privilege(line).as_deref(), expected, "{line}");
        }
    }

    #[test]
    fn command_paths_follow_the_perl_order() {
        assert_eq!(
            command_paths("/usr/bin/baz; echo bar"),
            vec!["/usr/bin/baz (plain script)".to_string()]
        );
        assert!(command_paths("        /usr/bin/foo) foobarbaz;;").is_empty());
        assert_eq!(
            command_paths("true `basename \"$0\"` `/usr/bin/foo bar`"),
            vec!["/usr/bin/foo (in backticks)".to_string()]
        );
        assert_eq!(
            command_paths("true `/usr/bin/foo \"$0\"`"),
            vec!["/usr/bin/foo (in backticks)".to_string()]
        );
        assert_eq!(
            command_paths("/usr/bin/gconftool-2 --makefile-install-rule foo.schema"),
            vec!["/usr/bin/gconftool-2 (plain script)".to_string()]
        );
        assert_eq!(
            command_paths(
                "DIVERSIONS=`env LC_ALL=C /usr/sbin/dpkg-divert --list | grep -E 'by amule(-utils)?$'` || true"
            ),
            vec!["/usr/sbin/dpkg-divert (in backticks)".to_string()]
        );
        assert_eq!(
            command_paths(
                "if [ ! -x /usr/sbin/dpkg-state-override ] ||    !  dpkg-state-override > /dev/null"
            ),
            vec!["/usr/sbin/dpkg-state-override (in test syntax)".to_string()]
        );
        assert_eq!(
            command_paths("/bin/foo; /usr/bin/bar"),
            vec!["/bin/foo (plain script)".to_string()]
        );
        assert!(
            command_paths("start-stop-daemon --start --quiet --name foo --startas /usr/bin/foo")
                .is_empty()
        );
        assert_eq!(
            command_paths("true ` /usr/bin/foo ` `/bin/bar; echo`"),
            vec![
                "/usr/bin/foo (in backticks)".to_string(),
                "/bin/bar (in backticks)".to_string(),
            ]
        );
    }

    #[test]
    fn set_e_follows_lintian_2_139() {
        for line in [
            "set -e",
            "set -eu",
            "set -u -e",
            "set -- -e",
            "    set -e",
            "foo; set -e",
            "set -euo pipefail",
            "! set -e",
            "env FOO=bar set -e",
        ] {
            assert!(SET_E.is_match(line), "{line}");
        }
        for line in ["set -o errexit", "echo set -e", "SET -e", "set -E"] {
            assert!(!SET_E.is_match(line), "{line}");
        }
    }

    #[test]
    fn env_sh_postinst_is_not_scanned_but_postrm_is() {
        let postinst = scan_entry(Path::new("postinst"), b"#!/usr/bin/env sh\nread foo\n");
        assert!(postinst.is_none());
        let postrm = scan_entry(Path::new("postrm"), b"#!/usr/bin/env sh\nread foo\n").unwrap();
        assert!(postrm.is_shell);
        assert!(line_is_read(&postrm.lines[0].text));
    }

    #[test]
    fn dash_is_scanned_and_pdksh_is_not() {
        assert!(scan_entry(Path::new("postinst"), b"#!/bin/dash\nread foo\n").is_some());
        assert!(scan_entry(Path::new("postinst"), b"#!/bin/pdksh\nread foo\n").is_none());
    }
}
