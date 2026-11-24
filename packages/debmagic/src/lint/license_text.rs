//! Text of Patched files as lintian's `cruft` Check reads it.
//!
//! Each text file is lower-cased and folded by [`clean_text`], then matched
//! against the table entries of lintian's `check_for_single_bad_license`
//! ([`BadLicense`]). Lintian's `full_text_check` reads one sliding window and
//! then leaves its loop unconditionally (`last BLOCK` at the end of the body),
//! so only the first window is matched: the first 32 KiB of the file, two
//! 16 KiB blocks. This module does the same ([`WINDOW`]).

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, OnceLock};

use regex::Regex;

use super::context::SourceContext;
use super::data::lines;
use super::file_tree::{EntryKind, FileTree};

/// Bytes probed for a NUL, as lintian's `-T` test reads the first block.
const BINARY_PROBE: usize = 512;

/// The bytes lintian matches: its first sliding window, which reads twice the
/// 16 KiB block size on the first call (`SlidingWindow::readwindow`).
const WINDOW: usize = 2 * 16_384;

/// A Patched file that reads as text.
pub(crate) struct TextFile {
    pub(crate) path: PathBuf,
    /// The lower-cased first window of the contents (lintian's `$lowercase`).
    pub(crate) lowercase: String,
    /// [`clean_text`] of `lowercase`, computed on first use (see [`Self::clean`]).
    clean: OnceLock<String>,
}

impl TextFile {
    pub(crate) fn new(path: PathBuf, lowercase: String) -> Self {
        Self {
            path,
            lowercase,
            clean: OnceLock::new(),
        }
    }

    /// lintian's `$clean`: [`clean_text`] of `lowercase`. Most files fail a
    /// keyword check before a Rule reads it, so the cleaning is paid only for
    /// the files that reach it.
    pub(crate) fn clean(&self) -> &str {
        self.clean.get_or_init(|| clean_text(&self.lowercase))
    }
}

static NON_FREE_SECTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:non-free|non-free-firmware|restricted|multiverse)/").expect("valid regex")
});

/// lintian's `processable->is_non_free`: the source `Section` (`main` when
/// absent) starts with `non-free/`, `non-free-firmware/`, `restricted/` or
/// `multiverse/`.
pub(crate) fn is_non_free(ctx: &SourceContext<'_>) -> bool {
    let section = ctx
        .debian_control()
        .and_then(|control| control.source().unfolded_value("Section"))
        .filter(|section| !section.is_empty())
        .unwrap_or_else(|| "main".to_string());
    NON_FREE_SECTION.is_match(&section)
}

/// The Patched text files of `ctx`'s Subject, built once and shared by every
/// Rule that matches them. Empty when the Patched files are unavailable.
pub(crate) fn shared_text_files(ctx: &mut SourceContext<'_>) -> Arc<Vec<TextFile>> {
    ctx.shared_once(|ctx| ctx.files().map_or_else(Vec::new, text_files))
}

/// The regular files of `files` outside `.pc/` that read as text, in tree
/// order, with their first [`WINDOW`] bytes. Hardlinks are not read, as lintian's `full_text_check` skips them.
pub(crate) fn text_files(files: &FileTree) -> Vec<TextFile> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .into_iter()
        .filter(|entry| !entry.path().starts_with(".pc"))
        .filter_map(|entry| {
            let window = files.read_prefix(entry, WINDOW).ok()?;
            if window[..window.len().min(BINARY_PROBE)].contains(&0) {
                return None;
            }
            let lowercase = String::from_utf8_lossy(&window).to_lowercase();
            Some(TextFile::new(entry.path().to_path_buf(), lowercase))
        })
        .collect()
}

/// lintian's `clean_text` steps, in order: Texinfo, pod, TeX, HTML and
/// comment markup folded to spaces, then all whitespace collapsed.
const CLEAN_TABLE: &[(&str, &str, &[&str])] = &[
    (r"(?:@[[:alpha:]]*?\{)?\s*gnu\s*\}", " gnu ", &[r"gnu"]),
    (r#"\\\*\([LR]""#, "\"", &[r"\*("]),
    (r"\\-", "-", &[r"\-"]),
    (r"\(&fdl;\)", " ", &[r"&fdl;"]),
    (r"&fsf;", "free software foundation", &[r"&fsf;"]),
    (r"&nbsp;", " ", &[r"&nbsp;"]),
    (r#"^\.\\""#, " ", &[r#".\""#]),
    (r#""\s?[\n\x0B\x0C\r]#~\s?""#, "", &[r"#~"]),
    (r"\\par\b", "", &[r"\par"]),
    (r"\\url\{[^}]*?\}", " ", &[r"\url{"]),
    (r"\\emph\{", " ", &[r"\emph{"]),
    (r"\\href\{[^}]*?\}\{([^}]*?)\}", " ${1} ", &[r"\href{"]),
    (
        r"\\hyperlink\{[^}]*?\}\{([^}]*?)\}",
        " ${1} ",
        &[r"\hyperlink{"],
    ),
    (r"-\\/", "-", &[r"-\/"]),
    (r"\\char", " ", &[r"\char"]),
    (r"@c(?:omment)?[ \t]+end[ \t]+ifman\s+", " ", &[r"@c"]),
    (r"@c(?:omment)?\s+noman\s+", " ", &[r"@c"]),
    (r"@c(?:omment)?\s+", " ", &[r"@c"]),
    (r"@[birt]\{", " ", &[r"@"]),
    (r"@sansserif\{", " ", &[r"@sansserif{"]),
    (r"@slanted\{", " ", &[r"@slanted{"]),
    (r"@var\{", " ", &[r"@var{"]),
    (r"@(?:small)?example\s+", " ", &[r"example"]),
    (r"@end[ \t]+(?:small)example\s+", " ", &[r"example"]),
    (r"@group\s+", " ", &[r"@group"]),
    (r"@end[ \t]+group\s+", " ", &[r"group"]),
    (r"<!--", " ", &[r"<!--"]),
    (r"-->", " ", &[r"-->"]),
    (r"</?a[^>]*?>", " ", &[r"<"]),
    (r"<br\s*/?>", " ", &[r"<br"]),
    (r"</?citetitle[^>]*?>", " ", &[r"<"]),
    (r"</?div[^>]*?>", " ", &[r"<"]),
    (r"</?font[^>]*?>", " ", &[r"<"]),
    (r"</?b[^>]*?>", " ", &[r"<"]),
    (r"</?i[^>]*?>", " ", &[r"<"]),
    (r"</?link[^>]*?>", " ", &[r"<"]),
    (r"</?p[^>]*?>", " ", &[r"<"]),
    (r"</?quote[^>]*?>", " ", &[r"<"]),
    (r"</?span[^>]*?>", " ", &[r"<"]),
    (r"</?ulink[^>]*?>", " ", &[r"<"]),
    (r"</?var[^>]*?>", " ", &[r"<"]),
    (r"&[lr]dquo;", " ", &[r"dquo;"]),
    (r"\(\*note.*?::\)", " ", &[r"(*note"]),
    (r#""\s*,"#, " ", &[r#"""#]),
    (r#",\s*""#, " ", &[r","]),
    (r"\\n", " ", &[r"\n"]),
    (r"\\&", " ", &[r"\&"]),
    (r"\\s(?:0|-1)", " ", &[r"\s"]),
    (r"``|''", " ", &[r"``", r"''"]),
    (r"^[-\+!<>]", " ", &[]),
    (r"@@\s*[-+]\d+,\d+\s+[-+]\d+,\d+\s*@@", " ", &[r"@@"]),
    (r"\}", " ", &[r"}"]),
    (r"^\s*\\(sub)*section\*?\{\s*\S+", " ", &[r"section"]),
    (r#"[%\*"\|\\#~]"#, " ", &[]),
    (r"\s+", " ", &[]),
];

/// One compiled step of [`CLEAN_TABLE`].
struct CleanStep {
    regex: Regex,
    replacement: &'static str,
    /// Literals of which every match of `regex` contains one. Empty when the
    /// step has no such literal, so it always runs.
    literals: &'static [&'static str],
}

static CLEAN_STEPS: LazyLock<Vec<CleanStep>> = LazyLock::new(|| {
    CLEAN_TABLE
        .iter()
        .map(|(pattern, replacement, literals)| CleanStep {
            regex: Regex::new(&format!("(?ms){pattern}")).expect("valid regex"),
            replacement,
            literals,
        })
        .collect()
});

/// lintian's `clean_text`: the lower-cased text with comment markers, markup
/// and line breaks folded into single spaces, trimmed.
///
/// A step is skipped when the text has none of its literals, as no match of
/// its pattern can then exist. A step that matches nothing keeps the text
/// borrowed, so no copy is made for it.
pub(crate) fn clean_text(lowercase: &str) -> String {
    let mut text = Cow::Borrowed(lowercase);
    for step in CLEAN_STEPS.iter() {
        if !step.literals.is_empty() && !step.literals.iter().any(|literal| text.contains(literal))
        {
            continue;
        }
        let replaced = step.regex.replace_all(&text, step.replacement);
        if let Cow::Owned(owned) = replaced {
            text = Cow::Owned(owned);
        }
    }
    text.trim().to_string()
}

/// One entry of lintian's license table: a file is flagged when it has a
/// keyword, a sentence and (if given) the regex matches its clean text.
pub(crate) struct BadLicense {
    keywords: &'static [&'static str],
    sentences: &'static [&'static str],
    regex: Option<Regex>,
}

impl BadLicense {
    /// `pattern` is lintian's Perl pattern under `/msx`: `[ ]` is a space and
    /// unescaped whitespace is ignored.
    pub(crate) fn new(
        keywords: &'static [&'static str],
        sentences: &'static [&'static str],
        pattern: Option<&str>,
    ) -> Self {
        let regex = pattern.map(|pattern| {
            let pattern = pattern.replace("[ ]", r"\x20");
            Regex::new(&format!("(?msx){pattern}")).expect("valid regex")
        });
        Self {
            keywords,
            sentences,
            regex,
        }
    }

    pub(crate) fn matches(&self, text: &TextFile) -> bool {
        self.keywords
            .iter()
            .any(|keyword| text.lowercase.contains(keyword))
            && self
                .sentences
                .iter()
                .any(|sentence| text.clean().contains(sentence))
            && self
                .regex
                .as_ref()
                .is_none_or(|regex| regex.is_match(text.clean()))
    }
}

static SCRIPT_WITH_SRC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\A<script\s+[^>]*?src="[^"]+?"[^>]*?>"#).expect("valid regex"));
static INLINE_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<script[^>]*?>(.*?)</script>").expect("valid regex"));

/// The contents of the inline `<script>` blocks of a lower-cased HTML page,
/// in order, as lintian's `cruft` Check walks them: a script with a `src`
/// attribute is skipped, and the walk stops at the first `<script` that has no
/// closing `</script>`.
pub(crate) fn inline_scripts(lowercase: &str) -> Vec<&str> {
    let mut rest = lowercase;
    let mut scripts = Vec::new();
    while let Some(at) = rest.find("<script") {
        rest = &rest[at..];
        if let Some(sourced) = SCRIPT_WITH_SRC.find(rest) {
            rest = &rest[sourced.end()..];
        } else if let Some(inline) = INLINE_SCRIPT.captures(rest) {
            let whole = inline.get(0).expect("whole match");
            scripts.push(inline.get(1).map_or("", |body| body.as_str()));
            rest = &rest[whole.end()..];
        } else {
            break;
        }
    }
    scripts
}

/// lintian's `_strip_c_comments`: `/* */` and `//` comments removed, string
/// and character literals kept. A `//` comment runs to the first newline that
/// no backslash escapes; one with no such newline after it is kept.
pub(crate) fn strip_c_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'*') => match find_bytes(bytes, i + 2, b"*/") {
                Some(end) => i = end + 2,
                None => {
                    out.push(b'/');
                    i += 1;
                }
            },
            b'/' if bytes.get(i + 1) == Some(&b'/') => match unescaped_newline(bytes, i + 2) {
                Some(newline) => i = newline,
                None => {
                    out.push(b'/');
                    i += 1;
                }
            },
            quote @ (b'"' | b'\'') => match literal_end(bytes, i, quote) {
                Some(end) => {
                    out.extend_from_slice(&bytes[i..end]);
                    i = end;
                }
                None => {
                    out.push(quote);
                    i += 1;
                }
            },
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn find_bytes(bytes: &[u8], from: usize, pattern: &[u8]) -> Option<usize> {
    bytes
        .get(from..)?
        .windows(pattern.len())
        .position(|window| window == pattern)
        .map(|offset| from + offset)
}

/// The first newline at or after `from` that the byte before does not escape.
fn unescaped_newline(bytes: &[u8], from: usize) -> Option<usize> {
    (from..bytes.len()).find(|&at| bytes[at] == b'\n' && bytes[at - 1] != b'\\')
}

/// The index just past the literal that opens at `start`, or `None` when it is
/// not closed. A backslash keeps the byte after it.
fn literal_end(bytes: &[u8], start: usize, quote: u8) -> Option<usize> {
    let mut at = start + 1;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            byte if byte == quote => return Some(at + 1),
            _ => at += 1,
        }
    }
    None
}

static JSLINT_BLOCK_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*/\*[^\n]*\*/\s*$").expect("valid regex"));
static JSLINT_LINE_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*//[^\n]*$").expect("valid regex"));
static LEADING_SPACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s+").expect("valid regex"));
static BLANK_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*\n").expect("valid regex"));

/// lintian's `$contiguous`: a script or a JavaScript file with its JSLint
/// banner lines, comments, indentation and blank lines removed, and its lines
/// joined with spaces. Browserify patterns are matched against this.
pub(crate) fn contiguous_code(lowercase: &str) -> String {
    let text = JSLINT_BLOCK_LINE.replace_all(lowercase, "");
    let text = JSLINT_LINE_COMMENT.replace_all(&text, "");
    let text = LEADING_SPACE.replace_all(&text, "");
    let text = strip_c_comments(&text);
    let text = BLANK_LINE.replace_all(&text, "");
    // `s/\n\Z//m`: one newline at the end goes, and the second-last one when
    // the text ends in two.
    let mut text = text.into_owned();
    if text.ends_with("\n\n") {
        text.remove(text.len() - 2);
    } else if text.ends_with('\n') {
        text.pop();
    }
    text.replace('\n', " ")
}

/// lintian's `cruft/browserify-regex` table, in file order: each pattern is
/// matched case-sensitively under `/msx`, and its first group is the code
/// fragment the hint names.
static BROWSERIFY: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    lines(include_str!("data/cruft/browserify-regex"))
        .filter_map(|line| line.split_once("~~"))
        .map(|(_, pattern)| {
            Regex::new(&format!("(?msx){}", pattern.trim())).expect("valid browserify regex")
        })
        .collect()
});

/// The extra of `source-contains-browserified-javascript` for `contiguous`
/// (see [`contiguous_code`]): `code fragment:` and the first group of the
/// first pattern that matches, or an empty extra when that group is absent.
/// `None` when no pattern matches.
pub(crate) fn browserify_extra(contiguous: &str) -> Option<String> {
    BROWSERIFY.iter().find_map(|pattern| {
        pattern.captures(contiguous).map(|captures| {
            captures.get(1).map_or_else(String::new, |fragment| {
                format!("code fragment:{}", fragment.as_str())
            })
        })
    })
}

static RFC_WHITELIST: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    lines(include_str!("data/cruft/rfc-whitelist"))
        .map(|pattern| Regex::new(&pattern.replace(r"\Z", r"\z")).expect("valid regex"))
        .collect()
});

/// lintian's `rfc_whitelist_filename`: `debian/copyright` is never flagged,
/// and neither is a file whose lower-cased basename matches `cruft/rfc-whitelist`.
pub(crate) fn rfc_is_exempt(path: &Path) -> bool {
    if path == Path::new("debian/copyright") {
        return true;
    }
    let basename = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    RFC_WHITELIST
        .iter()
        .any(|pattern| pattern.is_match(&basename))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(content: &str) -> TextFile {
        TextFile::new(PathBuf::from("file.c"), content.to_lowercase())
    }

    #[test]
    fn clean_text_folds_comment_markers_and_whitespace() {
        assert_eq!(
            clean_text("/*\n * unicode,  inc.\n * hereby\n */"),
            "/ unicode, inc. hereby /"
        );
        assert_eq!(clean_text("<p>hello</p>\n\"a\",\n"), "hello a");
        assert_eq!(clean_text("\\href{http://x}{name}"), "name");
    }

    /// Pins `clean_text` on inputs that hit each kind of step, so a faster
    /// implementation that changes any of them fails here before it reaches
    /// the lint output.
    #[test]
    fn clean_text_pins_representative_inputs() {
        let cases: &[(&str, &str)] = &[
            ("", ""),
            ("plain text", "plain text"),
            ("  leading and   trailing \n\n", "leading and trailing"),
            (
                "line one\nline two\r\nline three",
                "line one line two line three",
            ),
            ("@gnu{}", "@gnu{"),
            ("\\*(L\"quoted\"", "quoted"),
            ("non\\-breaking", "non-breaking"),
            (
                "the &fsf; and &nbsp;here",
                "the free software foundation and here",
            ),
            ("<p>hello</p>\n\"a\",\n", "hello a"),
            ("<a href=\"x\">link</a> and <br/>break", "link and break"),
            ("\\href{http://x}{name}", "name"),
            ("\\url{http://x} and \\emph{x}", "and x"),
            ("@c comment text", "comment text"),
            ("``quoted'' text", "quoted text"),
            (
                "/*\n * unicode,  inc.\n * hereby\n */",
                "/ unicode, inc. hereby /",
            ),
            ("\\section{Title}", ""),
            ("@@ -1,2 +3,4 @@", ""),
            ("a{b}c", "a{b c"),
        ];
        for (input, expected) in cases {
            assert_eq!(clean_text(input), *expected, "input {input:?}");
        }
    }

    #[test]
    fn bad_license_needs_keyword_sentence_and_regex() {
        let plain = BadLicense::new(&["evil"], &["good not evil"], None);
        assert!(plain.matches(&text("GOOD\n* NOT EVIL")));
        assert!(!plain.matches(&text("good only")));

        let with_regex = BadLicense::new(&["evil"], &["good not evil"], Some("good[ ]not[ ]evil"));
        assert!(with_regex.matches(&text("good not evil")));
        assert!(!with_regex.matches(&text("goodnot evil, evil")));
    }

    #[test]
    fn rfc_exemptions() {
        assert!(rfc_is_exempt(Path::new("debian/copyright")));
        assert!(rfc_is_exempt(Path::new("src/RFC2629.txt")));
        assert!(rfc_is_exempt(Path::new("doc/punycode.html")));
        assert!(!rfc_is_exempt(Path::new("src/rfc.txt")));
    }
}
