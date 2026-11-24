//! Lines of a `/bin/sh` script that lintian's `shell/non-posix/bash-centric`
//! Check reports as bash-only.
//!
//! The Check runs on Entries whose shebang basename is `sh`. Each logical
//! line (continuations joined, comments removed) is tested against about
//! sixty patterns. Lookaround and the `(*SKIP)(*F)` idiom are not in the
//! `regex` crate, so three patterns are small hand-written scanners. The
//! matched text is the first match on the unmodified line, as in Perl.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::interpreter::{Hashbang, basename};

/// lintian's `$LEADING_PATTERN`: where a shell command may begin.
const LEADING: &str = r#"(?:(?:^|[`&;(|{])\s*|(?:if|then|do|while|!)\s+|env(?:\s+[[:alnum:]_]+=(?:\S+|"[^"]*"|'[^']*'))*\s+)"#;

/// One pattern of the Check. Most are regexes; three need a scanner.
enum Test {
    Re(Regex),
    /// `"(?!…)…"(*SKIP)(*F)|<<<`: `<<<` outside a double-quoted string.
    HereString,
    /// `(?<![$(])\(\(.*\)\)`
    DoubleParen,
    /// `\[\[(?!:)`
    DoubleBracket,
}

impl Test {
    /// The leftmost match in `text`, as Perl's `($text =~ /($regex)/)`.
    fn find<'a>(&self, text: &'a str) -> Option<&'a str> {
        match self {
            Test::Re(regex) => regex.find(text).map(|found| found.as_str()),
            Test::HereString => here_string(text).map(|(start, end)| &text[start..end]),
            Test::DoubleParen => double_paren(text).map(|(start, end)| &text[start..end]),
            Test::DoubleBracket => double_bracket(text).map(|(start, end)| &text[start..end]),
        }
    }
}

fn re(pattern: &str) -> Test {
    Test::Re(Regex::new(pattern).expect("valid regex"))
}

fn lead(command: &str) -> String {
    format!("{LEADING}{command}")
}

/// `source`, `echo` and the other patterns that run on the unmodified line.
static SINGLE_QUOTE_TESTS: LazyLock<Vec<Test>> = LazyLock::new(|| {
    vec![
        re(&lead(r"echo\s+(?:-[^e\s]+\s+)?'[^']*(\\[abcEfnrtv0])+.*?'")),
        re(&lead(r#"source\s+["']?(?:\./|[/$\w~.-])\S*"#)),
    ]
});

/// Checked after quoted strings are blanked, but reported from the line.
static STRING_TESTS: LazyLock<Vec<Test>> = LazyLock::new(|| {
    vec![
        re(r"\$\[\w+\]"),
        re(r"\$\{\w+:\d+(?::\d+)?\}"),
        re(r"\$\{\w+(/.+?){1,2}\}"),
        re(r"\$\{#?\w+\[[0-9*@]+\]\}"),
        re(r"\$\{!\w+[@*]\}"),
        re(r"\$\{!\w+\}"),
        re(r"(\$\(|`)\s*<\s*\S+\s*([\)`])"),
        re(r"\$\{?RANDOM\}?\b"),
        re(r"\$\{?(OS|MACH)TYPE\}?\b"),
        re(r"\$\{?HOST(TYPE|NAME)\}?\b"),
        re(r"\$\{?DIRSTACK\}?\b"),
        re(r"\$\{?EUID\}?\b"),
        re(r"\$\{?UID\}?\b"),
        re(r"\$\{?SECONDS\}?\b"),
        re(r"\$\{?BASH_[A-Z]+\}?\b"),
        re(r"\$\{?SHELLOPTS\}?\b"),
        re(r"\$\{?PIPESTATUS\}?\b"),
        re(r"\$\{?SHLVL\}?\b"),
        Test::HereString,
        re(&lead(
            r#"echo\s+(?:-[^e\s]+\s+)?"[^"]*(\\[abcEfnrtv0])+.*?""#,
        )),
    ]
});

/// Checked after single-quoted strings are blanked.
static BASHISM_TESTS: LazyLock<Vec<Test>> = LazyLock::new(|| {
    vec![
        re(r"(?:^|\s+)function \w+(\s|\(|$)"),
        re(r"(test|-o|-a)\s*[^\s]+\s+==\s"),
        re(r"\[\s+[^\]]+\s+==\s"),
        re(r"\s(\|&)"),
        re(r"[^\\$]\{(?:[^\s\\}]*?,)+[^\\}\s]*\}"),
        re(r"(?:^|\s+)\w+\[\d+\]="),
        re(&lead(r"read\s+(?:-[a-qs-zA-Z\d-]+)")),
        re(&lead(r#"read\s*(?:-\w+\s*)*(?:".*?"|'.*?')?\s*(?:;|$)"#)),
        re(r"&>"),
        re(r"(<&|>&)\s*((-|\d+)[^\s;|)`&\\]|[^-\d\s]+)"),
        Test::DoubleBracket,
        re(&lead(r"select\s+\w+")),
        re(&lead(r"echo\s+(-n\s+)?-n?en?")),
        re(&lead(r"exec\s+-[acl]")),
        re(r"(?:^|\s+)let\s"),
        Test::DoubleParen,
        re(r"\$\[[^\]\[]+\]"),
        re(r"(\[|test)\s+-a"),
        re(r"/dev/(tcp|udp)"),
        re(&lead(r"\w+\+=")),
        re(&lead(r"suspend\s")),
        re(&lead(r"caller\s")),
        re(&lead(r"complete\s")),
        re(&lead(r"compgen\s")),
        re(&lead(r"declare\s")),
        re(&lead(r"typeset\s")),
        re(&lead(r"disown\s")),
        re(&lead(r"builtin\s")),
        re(&lead(r"set\s+-[BHT]+")),
        re(&lead(r"alias\s+-p")),
        re(&lead(r"unalias\s+-a")),
        re(&lead(r"local\s+-[a-zA-Z]+")),
        re(r"(?:^|\s+)\s*\(?\w*[^(\w\s]+\S*?\s*\(\)\s*([{|(]|$)"),
        re(&lead(r"(push|pop)d(\s|$)")),
        re(&lead(r"export\s+-[^p]")),
        re(&lead(r"ulimit(\s|$)")),
        re(&lead(r"shopt(\s|$)")),
        re(&lead(r"time\s")),
        re(&lead(r"dirs(\s|$)")),
        re(r"(?:^|\s+)[<>]\(.*?\)"),
        re(r"(?:^|\s+)readonly\s+-[af]"),
        re(&lead(r"(sh|\$\{?SHELL\}?) -[rD]")),
        re(&lead(r"(sh|\$\{?SHELL\}?) --\w+")),
        re(&lead(r"(sh|\$\{?SHELL\}?) [-+]O")),
    ]
});

static DOT_QUOTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\A\s*\.\s+(?:"[^"]+"|'[^']+')\s*(?:[&|<;]|\d?>|$)"#).expect("valid regex")
});
static DOT_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\.\s+[^\s;`:]+\s+([^\s;]+))").expect("valid regex"));
static DOT_EXTRA_SKIP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:&|\||<|\d?>)").expect("valid regex"));

static EXEC_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^exec\s").expect("valid regex"));
static EXEC_REDIRECT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r">&(?:\d+|-)$").expect("valid regex"));

static COMMENT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("valid regex"));
static SINGLE_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[^\\](?:\\\\)*)'(?:\\.|[^\\'])+'").expect("valid regex"));
static DOUBLE_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\](?:\\\\)*)"(?:\\.|[^\\"])+""#).expect("valid regex"));
static TRAILING_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[^\[\\])[\s&;()](#.*$)").expect("valid regex"));

/// Quoted quotes, removed first: `"'"`, `'"'`, and then single-quoted text.
static QUOTED_QUOTE_DOUBLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\'"])"'""#).expect("valid regex"));
static QUOTED_QUOTE_SINGLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\'"])'"'"#).expect("valid regex"));
static BLANK_SINGLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\"](?:\\\\)*)'(?:\\.|[^\\'])+'"#).expect("valid regex"));
static BLANK_DOUBLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\'](?:\\\\)*)"(?:\\.|[^\\"])+""#).expect("valid regex"));

/// One hint: the physical line that ends the logical line, and the extra.
pub(super) struct Finding {
    pub line: u32,
    pub extra: String,
}

/// Whether a file with this content is a `sh` script, lintian's
/// `basename($item->interpreter) eq 'sh'`.
pub(super) fn is_sh_script(content: &[u8]) -> bool {
    Hashbang::read(content).is_some_and(|hashbang| basename(&hashbang.interpreter()) == "sh")
}

/// Every hint for one script, in lintian's order. Stops at an `exec` that
/// does not redirect a file descriptor, as lintian does.
pub(super) fn findings(text: &str) -> Vec<Finding> {
    let mut found = Vec::new();
    let mut stashed = String::new();

    for (index, raw) in text.split_inclusive('\n').enumerate() {
        let position = index as u32 + 1;
        let raw = raw.strip_suffix('\n').unwrap_or(raw);

        if raw.trim().is_empty() || COMMENT_LINE.is_match(raw) {
            continue;
        }

        let mut no_comment = remove_comments(raw);
        if no_comment.ends_with('\\') {
            no_comment.pop();
            stashed.push_str(&no_comment);
            continue;
        }

        let line = std::mem::take(&mut stashed) + &no_comment;

        if EXEC_LINE.is_match(&line) && !EXEC_REDIRECT.is_match(&line) {
            break;
        }

        let mut unique: Vec<String> = Vec::new();
        for matched in check_line(&line) {
            if !unique.contains(&matched) {
                unique.push(matched);
            }
        }
        for matched in unique {
            found.push(Finding {
                line: position,
                extra: printable(&matched),
            });
        }
    }

    found
}

/// lintian's `check_line`: trimmed, non-empty matches in order.
fn check_line(line: &str) -> Vec<String> {
    let mut matches = Vec::new();

    if !DOT_QUOTED.is_match(line)
        && let Some(captures) = DOT_COMMAND.captures(line)
    {
        let dot_command = captures.get(1).map_or("", |found| found.as_str());
        let extra = captures.get(2).map_or("", |found| found.as_str());
        if !dot_command.is_empty() && !DOT_EXTRA_SKIP.is_match(extra) {
            matches.push(dot_command.to_string());
        }
    }

    for test in SINGLE_QUOTE_TESTS.iter() {
        if let Some(found) = test.find(line) {
            matches.push(found.to_string());
        }
    }

    let modified = QUOTED_QUOTE_DOUBLE.replace_all(line, "${1}");
    let modified = QUOTED_QUOTE_SINGLE.replace_all(&modified, "${1}");
    let modified = BLANK_SINGLE.replace_all(&modified, "${1}''");

    for test in STRING_TESTS.iter() {
        if test.find(&modified).is_some()
            && let Some(found) = test.find(line)
        {
            matches.push(found.to_string());
        }
    }

    let modified = BLANK_DOUBLE.replace_all(&modified, "${1}\"\"");

    for test in BASHISM_TESTS.iter() {
        if test.find(&modified).is_some()
            && let Some(found) = test.find(line)
        {
            matches.push(found.to_string());
        }
    }

    matches
        .into_iter()
        .map(|matched| matched.trim().to_string())
        .filter(|matched| !matched.is_empty())
        .collect()
}

/// lintian's quoting of a match: `'text'`, or `{hex:…}` when it holds a
/// control character.
fn printable(matched: &str) -> String {
    if matched.chars().any(char::is_control) {
        let hex: Vec<String> = matched
            .chars()
            .map(|character| format!("{:X}", character as u32))
            .collect();
        format!("{{hex:{}}}", hex.join("."))
    } else {
        format!("'{matched}'")
    }
}

/// lintian's `remove_comments`, with the same quoting rules.
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

/// `<<<` outside a double-quoted string. A string whose unescaped run before
/// its first quote or backslash holds `$(` or a backtick is not skipped.
fn here_string(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"'
            && !quote_guard_blocks(bytes, index)
            && let Some(end) = double_quoted_end(bytes, index)
        {
            index = end;
            continue;
        }
        if bytes[index..].starts_with(b"<<<") {
            return Some((index, index + 3));
        }
        index += 1;
    }
    None
}

/// Index after the closing quote of the string opened at `start`.
fn double_quoted_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return Some(index + 1),
            _ => index += 1,
        }
    }
    None
}

/// `(?![^"\\]*(?:\$\(|`))` at the opening quote at `start`.
fn quote_guard_blocks(bytes: &[u8], start: usize) -> bool {
    let run_end = bytes[start + 1..]
        .iter()
        .position(|&byte| byte == b'"' || byte == b'\\')
        .map_or(bytes.len(), |offset| start + 1 + offset);
    let run = &bytes[start + 1..run_end];
    run.windows(2).any(|pair| pair == b"$(") || run.contains(&b'`')
}

/// `(?<![$(])\(\(.*\)\)`: the leftmost `((` not preceded by `$` or `(`,
/// through the last `))` after it.
fn double_paren(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    for start in 0..bytes.len().saturating_sub(1) {
        if bytes[start] != b'(' || bytes[start + 1] != b'(' {
            continue;
        }
        if start > 0 && matches!(bytes[start - 1], b'$' | b'(') {
            continue;
        }
        let last = (start + 2..bytes.len().saturating_sub(1))
            .rev()
            .find(|&index| bytes[index] == b')' && bytes[index + 1] == b')');
        if let Some(index) = last {
            return Some((start, index + 2));
        }
    }
    None
}

/// `\[\[(?!:)`
fn double_bracket(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    (0..bytes.len().saturating_sub(1)).find_map(|start| {
        let opens = bytes[start] == b'[' && bytes[start + 1] == b'[';
        let colon_follows = bytes.get(start + 2) == Some(&b':');
        (opens && !colon_follows).then_some((start, start + 2))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `check_line` reports for each line, pinned on bashisms and on near
    /// misses, so that the pattern tables can change without changing output.
    #[test]
    fn reports_each_bashism_once_per_line() {
        let cases: [(&str, Vec<&str>); 16] = [
            ("function foo() {", vec!["function foo("]),
            ("if [[ $x == y ]]; then", vec!["[ $x ==", "[["]),
            ("echo hi |& cat", vec!["|&"]),
            ("read -p x var", vec!["read -p"]),
            ("echo hello", vec![]),
            ("cat foo >&2", vec![]),
            ("echo -n -e \"a\\tb\"", vec!["echo -n -e"]),
            // A non-ASCII letter before a bashism does not hide it.
            ("caf\u{e9}; function f() {", vec!["function f("]),
            // LATIN SMALL LETTER LONG S is not `s`, so `select` is not matched.
            ("\u{17f}elect foo", vec![]),
            ("let x=1", vec!["let"]),
            ("x=(a b)", vec![]),
            ("a=1 b=2 ; declare -a z", vec!["; declare"]),
            ("source ./x.sh", vec!["source ./x.sh"]),
            ("[ -a x ]", vec!["[ -a"]),
            ("echo ${x:0:2}", vec!["${x:0:2}"]),
            ("sh -O extglob x", vec!["sh -O"]),
        ];
        for (line, expected) in cases {
            assert_eq!(check_line(line), expected, "{line:?}");
        }
    }
}
