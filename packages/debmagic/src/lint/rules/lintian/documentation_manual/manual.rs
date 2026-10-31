//! Where an installed manual page sits, and how its name is split.
//!
//! Lintian's `documentation/manual` returns before this walk on a udeb.
//! Callers skip udebs themselves. Paths that are not UTF-8 are left out.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{Entry, EntryKind};

static SECTION_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*man(\d)/$").expect("valid regex"));
static LOCALE_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/([^/]+)/man\d/$").expect("valid regex"));
static MENTIONS_UNDOCUMENTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|/)undocumented").expect("valid regex"));
/// `undocumented.[237].gz` as a prefix. The captured section must be the
/// directory the page lives in (`usr/share/man/manN`).
static SAME_DIR_UNDOCUMENTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^undocumented\.([237])\.gz").expect("valid regex"));
static RELATIVE_UNDOCUMENTED: LazyLock<[Regex; 4]> = LazyLock::new(|| {
    [
        Regex::new(r"^\.\./man[237]/undocumented\.[237]\.gz$").expect("valid regex"),
        Regex::new(r"^\.\./\.\./man/man[237]/undocumented\.[237]\.gz$").expect("valid regex"),
        Regex::new(r"^\.\./\.\./\.\./share/man/man[237]/undocumented\.[237]\.gz$")
            .expect("valid regex"),
        Regex::new(r"^\.\./\.\./\.\./\.\./usr/share/man/man[237]/undocumented\.[237]\.gz$")
            .expect("valid regex"),
    ]
});

/// A file, hardlink, or symlink in a `usr/share/man/.../manN/` directory.
pub(super) struct SectionPage<'a> {
    pub file_name: &'a str,
    /// Perl `$page_path`, with the trailing slash.
    pub page_path: String,
    /// The directory between `usr/share/man` and `manN`, when there is one.
    pub language: Option<String>,
}

/// Lintian's `is_file`: a regular file or a hardlink.
pub(super) fn is_file(entry: &Entry) -> bool {
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
}

/// Gzip magic `1f 8b`. The rest of the member is not checked.
pub(super) fn is_gzip(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1f, 0x8b])
}

pub(super) fn section_page(entry: &Entry) -> Option<SectionPage<'_>> {
    if !is_leaf(entry) {
        return None;
    }
    let parent = parent_utf8(entry.path())?;
    let subdir = man_subdir(parent)?;
    if !SECTION_DIR.is_match(&subdir) {
        return None;
    }
    let language = LOCALE_DIR
        .captures(&subdir)
        .map(|captures| captures[1].to_string());
    let file_name = entry.path().file_name()?.to_str()?;
    Some(SectionPage {
        file_name,
        page_path: format!("{parent}/"),
        language,
    })
}

/// File, hardlink, or symlink nested under `usr/share/man/`, not directly in
/// that directory. The section directory is not required.
pub(super) fn nested_manual_path(entry: &Entry) -> Option<&str> {
    if !is_leaf(entry) {
        return None;
    }
    let path = entry.path().to_str()?;
    let parent = parent_utf8(entry.path())?;
    man_subdir(parent)?;
    Some(path)
}

/// Perl flags a path matching `/_build_` or `_tmp_buildd`.
pub(super) fn has_build_marker(path: &str) -> bool {
    path.contains("/_build_") || path.contains("_tmp_buildd")
}

/// Locale directory names a country. `pt_BR` and `zh_` plus two ASCII
/// capitals are the exceptions the Check allows.
pub(super) fn names_a_country(language: &str) -> bool {
    language.contains('_') && !allowed_locale(language)
}

/// Final `.`-separated component is `gz`.
///
/// Perl `split` drops trailing empty fields, so `foo.1.gz.` still counts.
/// A name with no dot uses the whole name: `gz` counts, `foo` does not.
pub(super) fn final_component_is_gz(name: &str) -> bool {
    dot_pieces(name).last().copied() == Some("gz")
}

/// Command a manual-page filename documents.
///
/// Drop a trailing `gz` component, then require the new final component to
/// start with a digit (the section, which may continue as in `1p`). The
/// remaining components joined by `.` are the command. `foo.1.gz` documents
/// `foo`; `foo.en.1.gz` documents `foo.en`; `simple.pod.gz` documents nothing.
pub(super) fn command_from_manual_name(name: &str) -> Option<String> {
    let mut pieces = dot_pieces(name);
    let extension = pieces.pop()?;
    if extension != "gz" {
        pieces.push(extension);
    }
    let section = pieces.pop()?;
    if pieces.is_empty() || !section.starts_with(|character: char| character.is_ascii_digit()) {
        return None;
    }
    Some(pieces.join("."))
}

/// The symlink target mentions `undocumented` and is not one of the targets
/// the Check accepts. `page_path` is the page's directory with a trailing slash.
pub(super) fn is_broken_undocumented_target(target: &str, page_path: &str) -> bool {
    if !MENTIONS_UNDOCUMENTED.is_match(target) {
        return false;
    }
    if let Some(captures) = SAME_DIR_UNDOCUMENTED.captures(target) {
        let section = &captures[1];
        if page_path.starts_with(&format!("usr/share/man/man{section}")) {
            return false;
        }
    }
    !RELATIVE_UNDOCUMENTED
        .iter()
        .any(|pattern| pattern.is_match(target))
}

fn allowed_locale(language: &str) -> bool {
    if language == "pt_BR" {
        return true;
    }
    let mut characters = language.chars();
    characters.next() == Some('z')
        && characters.next() == Some('h')
        && characters.next() == Some('_')
        && characters
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
        && characters
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
        && characters.next().is_none()
}

fn is_leaf(entry: &Entry) -> bool {
    matches!(
        entry.kind(),
        EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
    )
}

fn parent_utf8(path: &Path) -> Option<&str> {
    path.parent()?.to_str()
}

/// Perl `$page_path =~ m{^usr/share/man(/\S+)}`: something under `usr/share/man`
/// whose first character is not whitespace. The returned string is that
/// suffix with a trailing slash (`/de/man1/`).
fn man_subdir(parent: &str) -> Option<String> {
    let rest = parent.strip_prefix("usr/share/man")?;
    let mut characters = rest.chars();
    if characters.next() != Some('/') {
        return None;
    }
    if characters.next().is_none_or(char::is_whitespace) {
        return None;
    }
    Some(format!("{rest}/"))
}

/// Perl `split(/\./, $name)`: leading empties stay, trailing empties go.
fn dot_pieces(name: &str) -> Vec<&str> {
    let mut pieces: Vec<&str> = name.split('.').collect();
    while pieces.last().is_some_and(|piece| piece.is_empty()) {
        pieces.pop();
    }
    pieces
}
