//! Manual-page bytes for the content Tags in `documentation/manual`.
//!
//! The Check inlines these limits and patterns; it does not load a data file
//! for them. Udebs never reach this walk. Callers skip udebs themselves.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{EntryKind, FileTree};

use super::manual::{is_file, is_gzip, section_page};

/// Perl `$MINIMUM_SHARED_OBJECT_SIZE`. Opening `.so` and empty checks apply
/// only when the stored file is smaller than this.
const MINIMUM_SHARED_OBJECT_SIZE: usize = 256;
/// Perl `$WIDE_SCREEN`. `MANWIDTH` passed to `man`.
const WIDE_SCREEN: &str = "120";

static ENGLISH_HIERARCHY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/man/man\d/").expect("valid regex"));
static SO_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\.so\s+(.+)?$").expect("valid regex"));
static MAN_SECTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^man\d+$").expect("valid regex"));
static SO_PAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^/]+\.\d(?:\S+)?(?:\.gz)?$").expect("valid regex"));
static MAN_PARENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*)/man\d\b").expect("valid regex"));
static WHATIS_STDOUT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^[^:]+: "(.*)"$"#).expect("valid regex"));
static USELESS_WHATIS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\S+)\s+-\s+manual page for ").expect("valid regex"));
static TEMPLATE_WHATIS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\S+\s+-\s+programs? to do something").expect("valid regex"));
static REFORMATTING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Reformatting").expect("valid regex"));
static BLANK_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*$").expect("valid regex"));
static GZIP_TOOL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\bgzip\b").expect("valid regex"));
static ASIAN_ADJUST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"warning \[.*\]: cannot adjust line").expect("valid regex"));
static ASIAN_BREAK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"warning \[.*\]: can't break line").expect("valid regex"));
static WRAP_FAILURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r":(\d+): warning \[.*\]: (?:can't break|cannot adjust) line").expect("valid regex")
});
static URL_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:https?|ftp|file)://.+").expect("valid regex"));
static UE_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\.\s*UE\b").expect("valid regex"));
static POD_MACRO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"warning: (?:macro )?'(?:Tr|IX)' not defined").expect("valid regex")
});
static COVERAGE_NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\ADeep recursion on subroutine "[^"]+" at .*B/Deparse.pm line \d+"#)
        .expect("valid regex")
});

/// How the first non-comment line of a page smaller than 256 bytes reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Opening {
    /// Not a short `.so` request and not an empty page. Scanning continues.
    Body,
    Empty,
    So {
        bad: bool,
    },
}

impl Opening {
    /// The Check returns before lexgrog, `man`, and the line scan.
    pub(super) fn stops(&self) -> bool {
        !matches!(self, Self::Body)
    }
}

/// One installed manual page that is a regular file or hardlink.
pub(super) struct Page {
    pub path: PathBuf,
    pub language: Option<String>,
    /// Directory matches `/man/manN/`, the hierarchy lexgrog is allowed to see.
    pub english_hierarchy: bool,
    pub lines: Vec<String>,
    pub opening: Opening,
    /// Bytes as stored in the package, gzip included.
    pub stored: Vec<u8>,
}

/// `lexgrog` status and the description it printed.
pub(super) struct Whatis {
    pub status: i32,
    description: String,
}

impl Whatis {
    /// Exit status 2. Lintian still parses the description afterwards.
    pub(super) fn bad(&self) -> bool {
        self.status == 2
    }

    /// Status 0 or 2, and the description matches the useless-whatis pattern.
    pub(super) fn useless(&self) -> bool {
        self.parsed() && useless_whatis(&self.description)
    }

    /// Status 0 or 2, the useless pattern missed, and the dh_make template matched.
    pub(super) fn template(&self) -> bool {
        self.parsed()
            && !useless_whatis(&self.description)
            && TEMPLATE_WHATIS.is_match(&self.description)
    }

    fn parsed(&self) -> bool {
        self.status == 0 || self.status == 2
    }
}

pub(super) fn pages(files: &FileTree) -> Vec<Page> {
    let mut found = Vec::new();
    for entry in files.entries() {
        if !is_file(entry) {
            continue;
        }
        let Some(section) = section_page(entry) else {
            continue;
        };
        let Ok(stored) = files.read(entry) else {
            continue;
        };
        let plain = if is_gzip(&stored) {
            let Some(plain) = crate::lint::gzip::gunzip(&stored) else {
                continue;
            };
            plain
        } else {
            stored.clone()
        };
        let lines = split_man_lines(&plain);
        let opening = if stored.len() < MINIMUM_SHARED_OBJECT_SIZE {
            classify_opening(&lines)
        } else {
            Opening::Body
        };
        found.push(Page {
            path: entry.path().to_path_buf(),
            language: section.language,
            english_hierarchy: ENGLISH_HIERARCHY.is_match(&section.page_path),
            lines,
            opening,
            stored,
        });
    }
    found
}

/// Line numbers of `.SH "POD ERRORS"`, counting comment lines.
pub(super) fn pod_error_lines(lines: &[String]) -> Vec<u32> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| !line.starts_with(".\\\"") && *line == ".SH \"POD ERRORS\"")
        .map(|(index, _)| (index + 1) as u32)
        .collect()
}

/// Copy `usr/share/man/` into a temp directory so `man` can resolve `.so`.
pub(super) fn stage_manuals(files: &FileTree) -> Option<tempfile::TempDir> {
    let dir = tempfile::TempDir::new().ok()?;
    write_tree(files, dir.path()).ok()?;
    Some(dir)
}

/// Filtered `man --warnings` lines. The `u32` is the 1-based stderr index.
pub(super) fn man_warnings(root: &Path, page: &Page) -> Vec<(u32, String)> {
    let file = root.join(&page.path);
    let localdir = man_parent(&file);
    let Ok(output) = man_command(&file, &localdir).output() else {
        return Vec::new();
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    filter_man_stderr(
        &stderr,
        page.language.as_deref(),
        &page.lines,
        coverage_enabled(),
    )
}

/// Run `lexgrog` on the stored bytes. `None` when the tool cannot be started.
pub(super) fn whatis(page: &Page) -> Option<Whatis> {
    let dir = tempfile::TempDir::new().ok()?;
    let path = dir.path().join(&page.path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(&path, &page.stored).ok()?;
    let output = tool("lexgrog").arg(&path).output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Some(Whatis {
        status: output.status.code().unwrap_or(0),
        description: whatis_description(&stdout),
    })
}

fn classify_opening(lines: &[String]) -> Opening {
    let first = opening_line(lines);
    if first.is_empty() {
        return Opening::Empty;
    }
    match so_destination(&first) {
        SoDest::No => Opening::Body,
        SoDest::Missing => Opening::So { bad: true },
        SoDest::Path(dest) => Opening::So {
            bad: so_is_bad(dest),
        },
    }
}

enum SoDest<'a> {
    No,
    Missing,
    Path(&'a str),
}

fn so_destination(line: &str) -> SoDest<'_> {
    let Some(captures) = SO_LINE.captures(line) else {
        return SoDest::No;
    };
    match captures.get(1) {
        Some(dest) => SoDest::Path(dest.as_str()),
        None => SoDest::Missing,
    }
}

fn so_is_bad(dest: &str) -> bool {
    let Some((head, rest)) = slash_pair(dest) else {
        return true;
    };
    if MAN_SECTION.is_match(head) {
        return false;
    }
    let Some((_, page)) = slash_pair(rest) else {
        return true;
    };
    !SO_PAGE.is_match(page)
}

fn slash_pair(value: &str) -> Option<(&str, &str)> {
    let (head, tail) = value.split_once('/')?;
    if head.is_empty() || tail.is_empty() {
        None
    } else {
        Some((head, tail))
    }
}

/// Perl's do-while over leading `.\"` comments. `"0"` is false, like Perl.
fn opening_line(lines: &[String]) -> String {
    let mut index = 0;
    loop {
        let raw = lines.get(index).map(String::as_str).unwrap_or("");
        let first = if perl_truthy(raw) {
            raw.to_string()
        } else {
            String::new()
        };
        index += 1;
        let next_ok = lines.get(index).is_some_and(|line| perl_truthy(line));
        if first.starts_with(".\\\"") && next_ok {
            continue;
        }
        return first;
    }
}

/// `(?i)(\S+)\s+-\s+manual page for \1`. The `regex` crate has no backreferences.
fn useless_whatis(description: &str) -> bool {
    for captures in USELESS_WHATIS.captures_iter(description) {
        let Some(name) = captures.get(1) else {
            continue;
        };
        let Some(whole) = captures.get(0) else {
            continue;
        };
        let rest = &description[whole.end()..];
        let name = name.as_str();
        if rest.len() >= name.len() && rest[..name.len()].eq_ignore_ascii_case(name) {
            return true;
        }
    }
    false
}

fn perl_truthy(line: &str) -> bool {
    !line.is_empty() && line != "0"
}

/// Readline: a terminating newline does not add an extra line.
fn split_man_lines(bytes: &[u8]) -> Vec<String> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(bytes);
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    if bytes.last() == Some(&b'\n') {
        lines.pop();
    }
    lines
}

fn write_tree(files: &FileTree, root: &Path) -> std::io::Result<()> {
    for entry in files.entries() {
        let Some(text) = entry.path().to_str() else {
            continue;
        };
        if !text.starts_with("usr/share/man/") {
            continue;
        }
        let dest = root.join(entry.path());
        match entry.kind() {
            EntryKind::Directory => std::fs::create_dir_all(&dest)?,
            EntryKind::Symlink => {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                if let Some(target) = entry.link_target() {
                    let _ = std::os::unix::fs::symlink(target, &dest);
                }
            }
            EntryKind::RegularFile | EntryKind::Hardlink => {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let Ok(bytes) = files.read(entry) else {
                    continue;
                };
                std::fs::write(&dest, bytes)?;
            }
        }
    }
    Ok(())
}

fn man_parent(file: &Path) -> PathBuf {
    let parent = file.parent().unwrap_or(file);
    let text = parent.to_string_lossy();
    MAN_PARENT
        .captures(&text)
        .map(|captures| PathBuf::from(&captures[1]))
        .unwrap_or_else(|| parent.to_path_buf())
}

fn man_command(file: &Path, localdir: &Path) -> Command {
    let mut command = tool("man");
    command
        .args(["--warnings", "-E", "UTF-8", "-l", "-Tutf8", "-Z"])
        .arg(file)
        .env("MANROFFSEQ", "")
        .env("MANWIDTH", WIDE_SCREEN)
        .current_dir(localdir);
    command
}

fn tool(program: &str) -> Command {
    let mut command = Command::new(program);
    command
        .env_clear()
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::null());
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    if let Some(tmpdir) = std::env::var_os("TMPDIR") {
        command.env("TMPDIR", tmpdir);
    }
    command
}

fn coverage_enabled() -> bool {
    std::env::var("LINTIAN_COVERAGE").is_ok_and(|value| !value.is_empty())
}

fn filter_man_stderr(
    stderr: &str,
    language: Option<&str>,
    man_lines: &[String],
    coverage: bool,
) -> Vec<(u32, String)> {
    let mut kept = Vec::new();
    for (index, line) in split_stderr(stderr).into_iter().enumerate() {
        if skip_stderr_line(&line, language, man_lines, coverage) {
            continue;
        }
        kept.push(((index + 1) as u32, clean_groff_line(&line)));
    }
    kept
}

fn split_stderr(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

fn skip_stderr_line(
    line: &str,
    language: Option<&str>,
    man_lines: &[String],
    coverage: bool,
) -> bool {
    if coverage && COVERAGE_NOISE.is_match(line) {
        return true;
    }
    if REFORMATTING.is_match(line) || BLANK_LINE.is_match(line) || GZIP_TOOL.is_match(line) {
        return true;
    }
    if asian_language(language) && (ASIAN_ADJUST.is_match(line) || ASIAN_BREAK.is_match(line)) {
        return true;
    }
    if let Some(captures) = WRAP_FAILURE.captures(line)
        && let Ok(number) = captures[1].parse::<usize>()
        && man_line_is_url_or_ue(man_lines, number)
    {
        return true;
    }
    POD_MACRO.is_match(line)
}

fn asian_language(language: Option<&str>) -> bool {
    language.is_some_and(|language| {
        language.starts_with("ja") || language.starts_with("ko") || language.starts_with("zh")
    })
}

fn man_line_is_url_or_ue(lines: &[String], number: usize) -> bool {
    let line = if number == 0 {
        lines.last()
    } else {
        lines.get(number - 1)
    };
    line.is_some_and(|line| URL_LINE.is_match(line) || UE_LINE.is_match(line))
}

fn clean_groff_line(line: &str) -> String {
    let stripped = strip_colon_space(line);
    stripped
        .strip_prefix("<standard input>:")
        .unwrap_or(stripped)
        .to_string()
}

fn strip_colon_space(line: &str) -> &str {
    let Some(colon) = line.find(':') else {
        return line;
    };
    if colon == 0 {
        return line;
    }
    let after = &line[colon + 1..];
    after.strip_prefix(' ').unwrap_or(line)
}

fn whatis_description(stdout: &str) -> String {
    let body = stdout.strip_suffix('\n').unwrap_or(stdout);
    if let Some(captures) = WHATIS_STDOUT.captures(body)
        && let Some(description) = captures.get(1)
    {
        return description.as_str().to_string();
    }
    stdout.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opening(text: &str) -> Opening {
        let bytes = text.as_bytes();
        assert!(bytes.len() < MINIMUM_SHARED_OBJECT_SIZE);
        classify_opening(&split_man_lines(bytes))
    }

    #[test]
    fn opening_line_follows_the_check() {
        assert_eq!(opening(""), Opening::Empty);
        assert_eq!(opening("\n"), Opening::Empty);
        assert_eq!(opening("0\n"), Opening::Empty);
        assert_eq!(opening(".\\\" only a comment\n"), Opening::Body);
        assert_eq!(opening(".\\\" a\n.\\\" b\n"), Opening::Body);
        // An empty next line is false, so the comment stops the skip.
        assert_eq!(opening(".\\\" comment\n\n.so man1/foo.1\n"), Opening::Body);
        assert_eq!(opening(".so man1/foo.1.gz\n"), Opening::So { bad: false });
        assert_eq!(opening(".so man3/bar.3\n"), Opening::So { bad: false });
        assert_eq!(
            opening(".\\\" comment\n.so man1/foo.1\n"),
            Opening::So { bad: false }
        );
        assert_eq!(
            opening(".so de/man1/foo.1.gz\n"),
            Opening::So { bad: false }
        );
        assert_eq!(
            opening(".so de/man1/foo.1p.gz\n"),
            Opening::So { bad: false }
        );
        assert_eq!(opening(".so man1/foo\n"), Opening::So { bad: false });
        assert_eq!(
            opening(".so ../somewhere/invalid/manpage.1\n"),
            Opening::So { bad: true }
        );
        assert_eq!(opening(".so foo\n"), Opening::So { bad: true });
        assert_eq!(opening(".so de/foo.1\n"), Opening::So { bad: true });
        assert_eq!(
            opening(".so de/man1/not-a-page\n"),
            Opening::So { bad: true }
        );
        assert_eq!(opening(".so \n"), Opening::So { bad: true });
        assert_eq!(opening(".so\n"), Opening::Body);
        assert!(matches!(opening(".TH FOO 1\n"), Opening::Body));
    }

    #[test]
    fn size_gate_skips_the_opening_check() {
        let mut bytes = b".so nowhere\n".to_vec();
        bytes.resize(MINIMUM_SHARED_OBJECT_SIZE, b'x');
        let lines = split_man_lines(&bytes);
        assert_eq!(classify_opening(&lines), Opening::So { bad: true });
        let opening = if bytes.len() < MINIMUM_SHARED_OBJECT_SIZE {
            classify_opening(&lines)
        } else {
            Opening::Body
        };
        assert_eq!(opening, Opening::Body);
    }

    #[test]
    fn man_lines_keep_a_final_empty_record_only_when_present() {
        assert!(split_man_lines(b"").is_empty());
        assert_eq!(split_man_lines(b"\n"), vec![""]);
        assert_eq!(split_man_lines(b"a\n"), vec!["a"]);
        assert_eq!(split_man_lines(b"a\nb"), vec!["a", "b"]);
        assert_eq!(split_man_lines(b"a\n\n"), vec!["a", ""]);
    }

    #[test]
    fn pod_line_numbers_count_comments() {
        let lines =
            split_man_lines(b".\\\" comment\n.SH Header \"POD ERRORS\"\n.SH \"POD ERRORS\"\n");
        assert_eq!(pod_error_lines(&lines), vec![3]);
    }

    #[test]
    fn stderr_filters_match_the_check() {
        let dep = "troff:<standard input>:13: warning: name 'dep' not defined (possibly missing space after 'de')";
        let kept = filter_man_stderr(&format!("{dep}\n"), None, &[], false);
        assert_eq!(kept, vec![(1, dep.to_string())]);

        let mixed = "Reformatting foo\n\ngzip: stdin: unexpected end of file\nreal warning\n";
        assert_eq!(
            filter_man_stderr(mixed, None, &[], false),
            vec![(4, "real warning".to_string())]
        );

        let cleaned =
            filter_man_stderr("groff: <standard input>:1: warning: x\n", None, &[], false);
        assert_eq!(cleaned, vec![(1, "1: warning: x".to_string())]);

        let asian = "troff:<standard input>:1: warning [p 1]: cannot adjust line\n\
            troff:<standard input>:2: warning [p 1]: can't break line\n\
            troff:<standard input>:3: warning: name 'dep' not defined\n";
        assert_eq!(
            filter_man_stderr(asian, Some("ja"), &[], false),
            vec![(
                3,
                "troff:<standard input>:3: warning: name 'dep' not defined".to_string()
            )]
        );
        assert_eq!(filter_man_stderr(asian, Some("zh_CN"), &[], false).len(), 1);
        assert_eq!(filter_man_stderr(asian, None, &[], false).len(), 3);

        let mut lines = vec![".TH X 1".to_string(); 14];
        lines.push(".UE".to_string());
        let wrap = "troff:<standard input>:15: warning [page 1, line 10]: cannot adjust line; underset by 23n\n";
        assert!(filter_man_stderr(wrap, None, &lines, false).is_empty());
        lines[14] = "plain text".to_string();
        assert_eq!(filter_man_stderr(wrap, None, &lines, false).len(), 1);

        let macros = "warning: macro 'Tr' not defined\nwarning: 'IX' not defined\nwarning: name 'Tr' not defined\n";
        assert_eq!(
            filter_man_stderr(macros, None, &[], false),
            vec![(3, "name 'Tr' not defined".to_string())]
        );

        let noise = "Deep recursion on subroutine \"foo\" at Foo/B/Deparse.pm line 12\nkept\n";
        assert_eq!(filter_man_stderr(noise, None, &[], false).len(), 2);
        assert_eq!(
            filter_man_stderr(noise, None, &[], true),
            vec![(2, "kept".to_string())]
        );
    }

    #[test]
    fn whatis_patterns_follow_lexgrog_text() {
        let useless = Whatis {
            status: 0,
            description: "true - manual page for true 5.96".to_string(),
        };
        assert!(useless.useless());
        assert!(!useless.template());

        let folded = Whatis {
            status: 0,
            description: "TRUE - MANUAL PAGE FOR TRUE".to_string(),
        };
        assert!(folded.useless());

        let template = Whatis {
            status: 0,
            description: "program - program to do something".to_string(),
        };
        assert!(template.template());
        assert!(!template.useless());

        let plural = Whatis {
            status: 0,
            description: "foo - programs to do something".to_string(),
        };
        assert!(plural.template());

        let both = Whatis {
            status: 0,
            description: "program - manual page for program to do something".to_string(),
        };
        assert!(both.useless());
        assert!(!both.template());

        let bad = Whatis {
            status: 2,
            description: "/tmp/page: parse failed\n".to_string(),
        };
        assert!(bad.bad());
        assert!(!bad.useless());

        let other = Whatis {
            status: 1,
            description: "true - manual page for true".to_string(),
        };
        assert!(!other.bad());
        assert!(!other.useless());
        assert!(!other.template());
    }
}
