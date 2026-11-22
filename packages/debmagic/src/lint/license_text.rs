//! Text of Patched files as lintian's `cruft` Check reads it.
//!
//! Each text file is lower-cased and folded by [`clean_text`], then matched
//! against the table entries of lintian's `check_for_single_bad_license`
//! ([`BadLicense`]). Lintian reads 16 KiB blocks through a sliding window and
//! stops at the first hit of a file; this module reads each file whole, so a
//! match that straddles a block edge is found here and missed by lintian.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::context::SourceContext;
use super::data::lines;
use super::file_tree::{EntryKind, FileTree};

/// Bytes probed for a NUL, as lintian's `-T` test reads the first block.
const BINARY_PROBE: usize = 512;

/// A Patched file that reads as text.
pub(crate) struct TextFile {
    pub(crate) path: PathBuf,
    /// The lower-cased contents (lintian's `$lowercase`).
    pub(crate) lowercase: String,
    /// [`clean_text`] of `lowercase` (lintian's `$clean`).
    pub(crate) clean: String,
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

/// The regular files of `files` outside `.pc/` that read as text, in tree
/// order. Hardlinks are not read, as lintian's `full_text_check` skips them.
pub(crate) fn text_files(files: &FileTree) -> Vec<TextFile> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .into_iter()
        .filter(|entry| !entry.path().starts_with(".pc"))
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            if bytes[..bytes.len().min(BINARY_PROBE)].contains(&0) {
                return None;
            }
            let lowercase = String::from_utf8_lossy(&bytes).to_lowercase();
            let clean = clean_text(&lowercase);
            Some(TextFile {
                path: entry.path().to_path_buf(),
                lowercase,
                clean,
            })
        })
        .collect()
}

/// lintian's `clean_text` steps, in order: Texinfo, pod, TeX, HTML and
/// comment markup folded to spaces, then all whitespace collapsed.
const CLEAN_TABLE: &[(&str, &str)] = &[
    (r"(?:@[[:alpha:]]*?\{)?\s*gnu\s*\}", " gnu "),
    (r#"\\\*\([LR]""#, "\""),
    (r"\\-", "-"),
    (r"\(&fdl;\)", " "),
    (r"&fsf;", "free software foundation"),
    (r"&nbsp;", " "),
    (r#"^\.\\""#, " "),
    (r#""\s?[\n\x0B\x0C\r]#~\s?""#, ""),
    (r"\\par\b", ""),
    (r"\\url\{[^}]*?\}", " "),
    (r"\\emph\{", " "),
    (r"\\href\{[^}]*?\}\{([^}]*?)\}", " ${1} "),
    (r"\\hyperlink\{[^}]*?\}\{([^}]*?)\}", " ${1} "),
    (r"-\\/", "-"),
    (r"\\char", " "),
    (r"@c(?:omment)?[ \t]+end[ \t]+ifman\s+", " "),
    (r"@c(?:omment)?\s+noman\s+", " "),
    (r"@c(?:omment)?\s+", " "),
    (r"@[birt]\{", " "),
    (r"@sansserif\{", " "),
    (r"@slanted\{", " "),
    (r"@var\{", " "),
    (r"@(?:small)?example\s+", " "),
    (r"@end[ \t]+(?:small)example\s+", " "),
    (r"@group\s+", " "),
    (r"@end[ \t]+group\s+", " "),
    (r"<!--", " "),
    (r"-->", " "),
    (r"</?a[^>]*?>", " "),
    (r"<br\s*/?>", " "),
    (r"</?citetitle[^>]*?>", " "),
    (r"</?div[^>]*?>", " "),
    (r"</?font[^>]*?>", " "),
    (r"</?b[^>]*?>", " "),
    (r"</?i[^>]*?>", " "),
    (r"</?link[^>]*?>", " "),
    (r"</?p[^>]*?>", " "),
    (r"</?quote[^>]*?>", " "),
    (r"</?span[^>]*?>", " "),
    (r"</?ulink[^>]*?>", " "),
    (r"</?var[^>]*?>", " "),
    (r"&[lr]dquo;", " "),
    (r"\(\*note.*?::\)", " "),
    (r#""\s*,"#, " "),
    (r#",\s*""#, " "),
    (r"\\n", " "),
    (r"\\&", " "),
    (r"\\s(?:0|-1)", " "),
    (r"``|''", " "),
    (r"^[-\+!<>]", " "),
    (r"@@\s*[-+]\d+,\d+\s+[-+]\d+,\d+\s*@@", " "),
    (r"\}", " "),
    (r"^\s*\\(sub)*section\*?\{\s*\S+", " "),
    (r#"[%\*"\|\\#~]"#, " "),
    (r"\s+", " "),
];

static CLEAN_STEPS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    CLEAN_TABLE
        .iter()
        .map(|(pattern, replacement)| {
            (
                Regex::new(&format!("(?ms){pattern}")).expect("valid regex"),
                *replacement,
            )
        })
        .collect()
});

/// lintian's `clean_text`: the lower-cased text with comment markers, markup
/// and line breaks folded into single spaces, trimmed.
pub(crate) fn clean_text(lowercase: &str) -> String {
    let mut text = lowercase.to_string();
    for (pattern, replacement) in CLEAN_STEPS.iter() {
        text = pattern.replace_all(&text, *replacement).into_owned();
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
                .any(|sentence| text.clean.contains(sentence))
            && self
                .regex
                .as_ref()
                .is_none_or(|regex| regex.is_match(&text.clean))
    }
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
        let lowercase = content.to_lowercase();
        TextFile {
            path: PathBuf::from("file.c"),
            clean: clean_text(&lowercase),
            lowercase,
        }
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
