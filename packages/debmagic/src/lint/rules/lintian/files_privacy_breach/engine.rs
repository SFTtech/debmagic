//! Detection shared by the `files/privacy-breach` Rules.
//!
//! Port of lintian's `Files::PrivacyBreach`. One scan of the package's
//! HTML-like files yields every hit; each Rule reports the hits of its Tag.
//! The tables are lintian's `files/privacy-breaker-*` data files, pinned to
//! 2.139.0.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use regex::{Regex, RegexBuilder};

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::Severity;

const BLOCKSIZE: usize = 16_384;

const WEBSITES: &str = include_str!("../../../data/files/privacy-breaker-websites");
const FRAGMENTS: &str = include_str!("../../../data/files/privacy-breaker-fragments");
const TAG_ATTRS: &str = include_str!("../../../data/files/privacy-breaker-tag-attr");

/// One finding: lintian's Tag, its extras, and the file.
struct Hit {
    tag: String,
    extra: Vec<String>,
    path: PathBuf,
}

/// Report the hits of one Tag as Diagnostics.
pub(super) fn report(ctx: &mut BinaryPackageContext<'_>, tag: &str, severity: Severity) {
    let hits = scan(ctx);
    for hit in hits.iter().filter(|hit| hit.tag == tag) {
        ctx.diagnostic(
            // lintian shows a newline inside a matched tag as the two characters `\n`.
            Diagnostic::warn(hit.extra.join(" ").replace('\n', "\\n"))
                .with_severity(severity)
                .with_location(Location {
                    path: hit.path.clone(),
                    line: None,
                    column: None,
                }),
        );
    }
}

struct Website {
    tag: String,
    regex: Regex,
    suggest: Option<String>,
}

struct Fragment {
    keyword: String,
    regex: Regex,
    /// Perl's `(?!\()(?!<)` after the match; the regex crate has no lookahead.
    not_followed_by: &'static [char],
    tag: String,
}

struct TagAttr {
    keywords: Vec<Vec<String>>,
    regex: Regex,
}

struct Tables {
    websites: BTreeMap<String, Website>,
    fragments: Vec<Fragment>,
    tag_attrs: BTreeMap<String, TagAttr>,
}

/// Compile a Perl pattern that lintian applies with `/xsm`.
fn compile(pattern: &str) -> (Regex, &'static [char]) {
    let mut pattern = pattern.replace("(?!doxygen.*)", "");
    let mut not_followed_by: &'static [char] = &[];
    if let Some(stripped) = pattern.strip_suffix(r"(?!\()(?!<)") {
        pattern = stripped.to_string();
        not_followed_by = &['(', '<'];
    }
    let pattern = pattern.replace(r"\Z", r"\z").replace(r"\h", r"[ \t]");
    let regex = RegexBuilder::new(&pattern)
        .ignore_whitespace(true)
        .dot_matches_new_line(true)
        .multi_line(true)
        .build()
        .unwrap_or_else(|error| panic!("privacy-breach pattern {pattern:?}: {error}"));
    (regex, not_followed_by)
}

fn split_first<'a>(text: &'a str, separator: &str) -> (&'a str, Option<&'a str>) {
    match text.split_once(separator) {
        Some((head, tail)) => (head.trim(), Some(tail)),
        None => (text.trim(), None),
    }
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let mut websites = BTreeMap::new();
    for line in crate::lint::data::lines(WEBSITES) {
        let (key, value) = split_first(line, "~~");
        let mut fields = value.unwrap_or("").splitn(3, "~~");
        let pattern = fields.next().unwrap_or("").trim();
        let tag = fields.next().map(str::trim).unwrap_or("");
        let suggest = fields.next().map(|suggest| suggest.trim().to_string());
        websites.insert(
            key.to_string(),
            Website {
                tag: if tag.is_empty() { key } else { tag }.to_string(),
                regex: compile(pattern).0,
                suggest,
            },
        );
    }
    let mut fragments = Vec::new();
    for line in crate::lint::data::lines(FRAGMENTS) {
        let (key, value) = split_first(line, "~~");
        let (pattern, tag) = split_first(value.unwrap_or(""), "~~");
        let (regex, not_followed_by) = compile(pattern);
        fragments.push(Fragment {
            keyword: key.to_string(),
            regex,
            not_followed_by,
            tag: tag.unwrap_or("").trim().to_string(),
        });
    }
    fragments.sort_by(|left, right| left.keyword.cmp(&right.keyword));
    let mut tag_attrs = BTreeMap::new();
    for line in crate::lint::data::lines(TAG_ATTRS) {
        let (key, value) = split_first(line, "~~");
        let (keywords, pattern) = split_first(value.unwrap_or(""), "~~");
        let pattern = pattern
            .unwrap_or("")
            .trim()
            .replace("&URL", r#"(?:(?:ht|f)tps?:)?//[^"\r\n]*"#);
        let keywords = keywords
            .split("||")
            .map(|group| group.split("&&").map(|k| k.trim().to_string()).collect())
            .collect();
        tag_attrs.insert(
            key.to_string(),
            TagAttr {
                keywords,
                regex: compile(&pattern).0,
            },
        );
    }
    Tables {
        websites,
        fragments,
        tag_attrs,
    }
});

static HTML_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(?:x?html?\d?|js|xht|xml|css)$").expect("valid regex"));
static HTML_COMMENT_END: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"--\s*>").expect("valid regex"));
static C_COMMENT_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*/").expect("valid regex"));
static LINK_REL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?isx)<link (?:\s[^>]+)? \s+ rel="([^"\r\n]*)" [^>]*>"#).expect("valid regex")
});
static MALLARD_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r".xml$").expect("valid regex"));
static LOCALHOST: LazyLock<[Regex; 3]> = LazyLock::new(|| {
    [
        r"(?i)^(?:[^/]+@)?localhost(?:[:][^/]+)?/",
        r"(?i)^(?:[^/]+@)?::1(?:[:][^/]+)?/",
        r"(?i)^(?:[^/]+@)?127(?:\.\d{1,3}){3}(?:[:][^/]+)?/",
    ]
    .map(|pattern| Regex::new(pattern).expect("valid regex"))
});
static CREATIVE_COMMONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^creativecommons.org/licenses/").expect("valid regex"));

const ALLOWED_REL: &[&str] = &[
    "alternate",
    "author",
    "bookmark",
    "canonical",
    "copyright",
    "edituri",
    "generator",
    "generator-home",
    "help",
    "license",
    "next",
    "prev",
    "schema.dct",
    "search",
];

const GENERIC_TRIGGERS: &[&str] = &[
    "src=\"http",
    "src=\"ftp",
    "src=\"//",
    "data-href=\"http",
    "data-href=\"ftp",
    "data-href=\"//",
    "codebase=\"http",
    "codebase=\"ftp",
    "codebase=\"//",
    "data=\"http",
    "data=\"ftp",
    "data=\"//",
    "poster=\"http",
    "poster=\"ftp",
    "poster=\"//",
    "<link",
    "@import",
];

fn scan(ctx: &mut BinaryPackageContext<'_>) -> Arc<Vec<Hit>> {
    ctx.shared_once(|ctx| {
        let josm = ctx.source_name() == "josm";
        let mut hits = Vec::new();
        let Some(files) = ctx.data_files() else {
            return hits;
        };
        for entry in files.entries() {
            if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                continue;
            }
            let path = entry.path().to_path_buf();
            let name = path.to_string_lossy();
            if !HTML_NAME.is_match(&name) {
                continue;
            }
            let basename = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if josm && basename == "defaultpresets.xml" {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            detect(&path, &basename, &bytes, &mut hits);
        }
        hits
    })
}

/// Lintian's sliding windows over the lower-cased file.
fn windows(lower: &[u8]) -> Vec<Vec<u8>> {
    let mut windows = Vec::new();
    if lower.is_empty() {
        return windows;
    }
    let first = lower.len().min(2 * BLOCKSIZE);
    windows.push(lower[..first].to_vec());
    let mut previous: &[u8] = if first > BLOCKSIZE {
        &lower[BLOCKSIZE..first]
    } else {
        &[]
    };
    let mut position = first;
    while position < lower.len() {
        let end = lower.len().min(position + BLOCKSIZE);
        let block = &lower[position..end];
        let mut window = previous.to_vec();
        window.extend_from_slice(block);
        windows.push(window);
        previous = block;
        position = end;
    }
    windows
}

/// Remove `<!-- -->` (not conditional comments) and `/* */` (not `@cc_on`).
fn strip_comments(text: &str) -> String {
    let text = strip_delimited(text, "<!--", "[if", &HTML_COMMENT_END);
    strip_delimited(&text, "/*", "@cc_on", &C_COMMENT_END)
}

fn strip_delimited(text: &str, open: &str, forbidden: &str, close: &Regex) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(open) {
        let after = &rest[start + open.len()..];
        if after.starts_with(forbidden) {
            out.push_str(&rest[..start + 1]);
            rest = &rest[start + 1..];
            continue;
        }
        match close.find(after) {
            Some(end) => {
                out.push_str(&rest[..start]);
                rest = &after[end.end()..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

fn detect(path: &Path, basename: &str, bytes: &[u8], hits: &mut Vec<Hit>) {
    let lower = bytes.to_ascii_lowercase();
    let mut seen: HashSet<String> = HashSet::new();
    let tables = &*TABLES;
    for window in windows(&lower) {
        let text = strip_comments(&String::from_utf8_lossy(&window));
        for fragment in &tables.fragments {
            if !text.contains(&fragment.keyword) {
                continue;
            }
            let Some(found) = fragment_match(fragment, &text) else {
                continue;
            };
            if seen.insert(format!("tag-{}", fragment.tag)) {
                hits.push(Hit {
                    tag: fragment.tag.clone(),
                    extra: vec![format!("(choke on: {found})")],
                    path: path.to_path_buf(),
                });
            }
        }
        if GENERIC_TRIGGERS
            .iter()
            .any(|trigger| text.contains(trigger))
        {
            detect_generic(&text, path, basename, bytes, &mut seen, hits);
        }
    }
}

fn fragment_match<'t>(fragment: &Fragment, text: &'t str) -> Option<&'t str> {
    fragment
        .regex
        .find_iter(text)
        .find(|found| {
            fragment.not_followed_by.is_empty()
                || !text[found.end()..]
                    .chars()
                    .next()
                    .is_some_and(|next| fragment.not_followed_by.contains(&next))
        })
        .map(|found| found.as_str())
}

fn detect_generic(
    text: &str,
    path: &Path,
    basename: &str,
    bytes: &[u8],
    seen: &mut HashSet<String>,
    hits: &mut Vec<Hit>,
) {
    for attr in TABLES.tag_attrs.values() {
        let triggered = attr
            .keywords
            .iter()
            .any(|group| group.iter().all(|keyword| text.contains(keyword.as_str())));
        if !triggered {
            continue;
        }
        for captures in attr.regex.captures_iter(text) {
            let (Some(full), Some(element), Some(url)) =
                (captures.get(1), captures.get(2), captures.get(3))
            else {
                continue;
            };
            let tag = Reference {
                full: full.as_str(),
                element: element.as_str(),
                url: url.as_str(),
            };
            check_url(&tag, path, basename, bytes, seen, hits);
        }
    }
}

/// A matched external reference: the whole element, its name, and the URL.
struct Reference<'a> {
    full: &'a str,
    element: &'a str,
    url: &'a str,
}

fn check_url(
    reference: &Reference<'_>,
    path: &Path,
    basename: &str,
    bytes: &[u8],
    seen: &mut HashSet<String>,
    hits: &mut Vec<Hit>,
) {
    let Reference { full, element, url } = *reference;
    let website = url.strip_prefix('"').unwrap_or(url);
    let website = ["https:", "http:", "ftps:", "ftp:"]
        .iter()
        .find_map(|scheme| website.strip_prefix(scheme))
        .unwrap_or(website);
    let website = website.strip_prefix("//").unwrap_or(website);
    let website = website.strip_suffix('"').unwrap_or(website);

    if LOCALHOST.iter().any(|regex| regex.is_match(website)) {
        return;
    }

    if element == "link"
        && let Some(rel) = LINK_REL.captures(full).and_then(|found| found.get(1))
        && ALLOWED_REL.contains(&rel.as_str())
    {
        return;
    }

    if basename == "legal.xml" && element == "link" && CREATIVE_COMMONS.is_match(website) {
        return;
    }

    if MALLARD_NAME.is_match(basename)
        && element == "link"
        && contains(bytes, br#" xmlns="http://projectmallard.org/1.0/""#)
    {
        return;
    }

    for (key, value) in &TABLES.websites {
        if value.regex.is_match(website) {
            if seen.insert(format!("tag-{key}")) {
                let mut extra = Vec::new();
                if let Some(suggest) = value.suggest.as_ref().filter(|s| !s.is_empty()) {
                    extra.push(suggest.clone());
                }
                extra.push(format!("({url})"));
                hits.push(Hit {
                    tag: value.tag.clone(),
                    extra,
                    path: path.to_path_buf(),
                });
            }
            return;
        }
    }

    if seen.insert(format!("tag-generic-{website}")) {
        hits.push(Hit {
            tag: "privacy-breach-generic".to_string(),
            extra: vec![format!("[{full}]"), format!("({url})")],
            path: path.to_path_buf(),
        });
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_compile() {
        assert!(!TABLES.websites.is_empty());
        assert!(!TABLES.fragments.is_empty());
        assert!(!TABLES.tag_attrs.is_empty());
    }

    #[test]
    fn first_window_is_double_block() {
        let data = vec![b'a'; 3 * BLOCKSIZE + 5];
        let windows = windows(&data);
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].len(), 2 * BLOCKSIZE);
        assert_eq!(windows[1].len(), 2 * BLOCKSIZE);
        assert_eq!(windows[2].len(), BLOCKSIZE + 5);
    }
}
