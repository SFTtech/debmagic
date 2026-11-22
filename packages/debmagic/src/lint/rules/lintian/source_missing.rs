//! What the Rules of lintian's `files/source-missing` Check share: which files
//! it visits, the byte sniffs that stand in for `file(1)`, and `find_source`.
//!
//! lintian's Check walks the Patched files once and reports several Tags per
//! file. Here each Tag is a Rule, so each Rule asks one question of an Entry:
//! [`is_prebuilt_javascript`], [`is_prebuilt_python`], [`source_is_missing`].
//! The questions follow the Check's control flow, including its early returns.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::follow_symlinks;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::file_type::FileType;

/// `$VERY_LONG_LINE_LENGTH`: a longer line marks a script or page as generated.
const VERY_LONG_LINE_LENGTH: usize = 512;

/// Bytes read to sniff a magic number.
const MAGIC_LEN: usize = 8;

/// The regular files and hardlinks of `files` the Check visits: lintian's
/// `visit_patched_files` drops quilt's `.pc/`, and the Check itself skips
/// `debian/missing-sources/`, where the sources it looks for are kept.
pub(super) fn visited_files(files: &FileTree) -> impl Iterator<Item = &Entry> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| {
            let path = entry.path();
            !path.starts_with(".pc")
                && !path
                    .parent()
                    .is_some_and(|dir| dir.starts_with("debian/missing-sources"))
        })
}

static JS_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.js$").expect("valid regex"));
static HTML_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(?:x?html?\d?|xht)$").expect("valid regex"));
/// A name that says the script is minified, packed or otherwise built.
static PREBUILT_JS_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[-._](?:compiled|compressed|lite|min|pack(?:ed)?|prod|umd|yc)\.js$")
        .expect("valid regex")
});
static DEPLOY_JAVA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:\A|[\n\x0B\x0C\r\x{85}\x{2028}\x{2029}])\s*var\s+deployJava\s*=\s*function")
        .expect("valid regex")
});

fn name_bytes(entry: &Entry) -> String {
    entry.path().to_string_lossy().into_owned()
}

fn base_name(entry: &Entry) -> String {
    entry
        .path()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// lintian's `file_type =~ m{^python \s \d(\.\d+)? \s byte-compiled}x`: a
/// CPython code object file, which starts with a 16-bit magic number and
/// `\r\n`. The numbers are those of CPython 1.5 to 3.14 as `file(1)` knows
/// them; a file of another release reads as data there.
pub(super) fn is_python_byte_code(prefix: &[u8]) -> bool {
    let [low, high, b'\r', b'\n', ..] = *prefix else {
        return false;
    };
    let magic = u16::from_le_bytes([low, high]);
    matches!(
        magic,
        // 1.5, 1.6, 2.0, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7
        20121 | 50428 | 50823 | 60202 | 60717 | 62011..=62021 | 62041..=62061
            | 62071..=62131 | 62151..=62161 | 62171..=62211
            // 3.0 to 3.14
            | 3000..=3999
    )
}

/// lintian's `file_type =~ m{^Macromedia \s Flash}x`: a Flash movie (`FWS`,
/// compressed `CWS` and `ZWS`) or a Flash video (`FLV\x01`).
fn is_macromedia_flash(prefix: &[u8]) -> bool {
    prefix.starts_with(b"FWS")
        || prefix.starts_with(b"CWS")
        || prefix.starts_with(b"ZWS")
        || prefix.starts_with(b"FLV\x01")
}

/// lintian's `file_type =~ m{^Composite \s Document \s File}x`: an OLE2
/// container, the format of a Flash project.
fn is_composite_document(prefix: &[u8]) -> bool {
    prefix.starts_with(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1")
}

fn prefix(files: &FileTree, entry: &Entry) -> Vec<u8> {
    files.read_prefix(entry, MAGIC_LEN).unwrap_or_default()
}

/// A Python byte-code file: the Tag `source-contains-prebuilt-python-object`.
pub(super) fn is_prebuilt_python(files: &FileTree, entry: &Entry) -> bool {
    is_python_byte_code(&prefix(files, entry))
}

/// The text of a file when it is UTF-8, as lintian's `decoded_utf8`.
fn decoded_utf8(files: &FileTree, entry: &Entry) -> Option<String> {
    String::from_utf8(files.read(entry).ok()?).ok()
}

/// `deployJava.js` that holds the Oracle `deployJava` library.
fn is_deploy_java(files: &FileTree, entry: &Entry) -> bool {
    base_name(entry) == "deployJava.js"
        && decoded_utf8(files, entry).is_some_and(|text| DEPLOY_JAVA.is_match(&text.to_lowercase()))
}

/// Whether the longest line of the file is longer than lintian's limit.
fn has_very_long_line(files: &FileTree, entry: &Entry) -> bool {
    files.read(entry).is_ok_and(|bytes| {
        bytes
            .split(|byte| *byte == b'\n')
            .any(|line| line.len() > VERY_LONG_LINE_LENGTH)
    })
}

/// A script that is minified, by its name or by its very long lines: the Tag
/// `source-contains-prebuilt-javascript-object`. The Oracle library
/// `deployJava.js` is not (the Check returns before it looks at the name).
pub(super) fn is_prebuilt_javascript(files: &FileTree, entry: &Entry) -> bool {
    if is_deploy_java(files, entry) {
        return false;
    }
    if PREBUILT_JS_NAME.is_match(&name_bytes(entry)) {
        return true;
    }
    JS_SUFFIX.is_match(&base_name(entry)) && has_very_long_line(files, entry)
}

/// A replacement for a regular expression of lintian's `find_source` table:
/// `pattern` (empty: none) is replaced once in the file's basename.
struct Alternative {
    replacement: &'static str,
    pattern: Option<Regex>,
}

fn alternatives(table: &[(&'static str, &str)]) -> Vec<Alternative> {
    table
        .iter()
        .map(|(replacement, pattern)| Alternative {
            replacement,
            pattern: (!pattern.is_empty())
                .then(|| Regex::new(pattern).expect("valid find_source pattern")),
        })
        .collect()
}

/// The source of a Python byte-code file.
static PYTHON: LazyLock<Vec<Alternative>> =
    LazyLock::new(|| alternatives(&[(".py", r"(?i)(?:\.cpython-\d{2}|\.pypy)?\.py[co]$")]));

/// The source of an ELF object: a source file with the build suffix cut off.
static ELF: LazyLock<Vec<Alternative>> = LazyLock::new(|| {
    let pattern =
        r"(?i)(?:[\.-](?:bin|elf|e|hs|linux\d+|oo?|or|out|so(?:\.\d+)*)|static|_o\.golden)?$";
    let table: Vec<(&'static str, &str)> = [
        ".asm", ".c", ".cc", ".cpp", ".cxx", ".f", ".F", ".i", ".ml", ".rc", ".S",
    ]
    .into_iter()
    .map(|replacement| (replacement, pattern))
    .collect();
    alternatives(&table)
});

static FLASH_OBJECT: LazyLock<Vec<Alternative>> =
    LazyLock::new(|| alternatives(&[(".as", r"(?i)\.swf$")]));

static FLASH_PROJECT: LazyLock<Vec<Alternative>> =
    LazyLock::new(|| alternatives(&[(".as", r"(?i)\.fla$")]));

static DEPLOY_JAVA_SOURCE: LazyLock<Vec<Alternative>> =
    LazyLock::new(|| alternatives(&[(".txt", r"(?i)\.js$"), ("", "")]));

static PREBUILT_JS_SOURCE: LazyLock<Vec<Alternative>> = LazyLock::new(|| {
    let pattern = r"(?i)(?:[-._](?:compiled|compressed|lite|min|pack(?:ed)?|prod|umd|yc))?\.js$";
    let table: Vec<(&'static str, &str)> = [
        ".js",
        "_orig.js",
        ".js.orig",
        ".src.js",
        "-src.js",
        ".debug.js",
        "-debug.js",
        "-nc.js",
    ]
    .into_iter()
    .map(|replacement| (replacement, pattern))
    .collect();
    alternatives(&table)
});

static LONG_LINE_JS_SOURCE: LazyLock<Vec<Alternative>> = LazyLock::new(|| {
    alternatives(&[
        (".debug.js", r"(?i)\.js$"),
        ("-debug.js", r"(?i)\.js$"),
        ("", ""),
    ])
});

static HTML_SOURCE: LazyLock<Vec<Alternative>> =
    LazyLock::new(|| alternatives(&[(".fragment.js", "$")]));

/// The path `request` names when resolved from `base` (a directory, relative
/// to the root), following `.` and `..` and a final symlink, as lintian's
/// `resolve_path`. `None` when it does not exist or leaves the tree. Symlinks
/// in the middle of the path are not followed.
fn resolve<'t>(tree: &'t FileTree, base: &Path, request: &str) -> Option<&'t Entry> {
    let mut parts: Vec<&std::ffi::OsStr> = base.iter().collect();
    for segment in request.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(std::ffi::OsStr::new(name)),
        }
    }
    let path: PathBuf = parts.into_iter().collect();
    let entry = tree.get(&path)?;
    if entry.kind() == EntryKind::Symlink {
        return follow_symlinks(tree, entry);
    }
    Some(entry)
}

/// lintian's `find_source`: whether a source for `item` is somewhere lintian
/// looks. Like lintian, it says "not found" for a hardlink, and for every file
/// of a non-free package (`find_source` returns `undef` in both cases, which
/// the Check treats as a missing source).
fn find_source(files: &FileTree, item: &Entry, non_free: bool, table: &[Alternative]) -> bool {
    if item.kind() != EntryKind::RegularFile || non_free {
        return false;
    }
    let base = base_name(item);
    let names: Vec<String> = table
        .iter()
        .filter_map(|alternative| {
            let name = match &alternative.pattern {
                Some(pattern) => pattern
                    .replace(&base, regex::NoExpand(alternative.replacement))
                    .into_owned(),
                None => base.clone(),
            };
            (!name.is_empty()).then_some(name)
        })
        .collect();

    let item_path = item.path();
    let dir = item_path.parent().unwrap_or(Path::new(""));
    let parent_name = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir_text = dir.to_string_lossy();

    let root = Path::new("");
    let found = |entry: Option<&Entry>| entry.is_some_and(|entry| entry.path() != item_path);

    // Standard locations.
    let missing_sources = Path::new("debian/missing-sources");
    if found(tree_get(files, &missing_sources.join(item_path)))
        || found(tree_get(files, &missing_sources.join(&base)))
    {
        return true;
    }
    for name in &names {
        for absolute in [
            ".libs".to_string(),
            format!(".libs/{dir_text}"),
            "unpacked".to_string(),
            "debian".to_string(),
            "debian/missing-sources".to_string(),
            format!("debian/missing-sources/{dir_text}"),
        ] {
            if found(resolve(files, root, &format!("{absolute}/{name}"))) {
                return true;
            }
        }
    }
    for name in &names {
        for relative in [
            ".".to_string(),
            "..".to_string(),
            "./src".to_string(),
            "../src".to_string(),
            format!("../../src/{parent_name}"),
            "./flash-src/src/net/gimite/websocket".to_string(),
        ] {
            if found(resolve(files, dir, &format!("{relative}/{name}"))) {
                return true;
            }
        }
    }
    false
}

fn tree_get<'t>(files: &'t FileTree, path: &Path) -> Option<&'t Entry> {
    let entry = files.get(path)?;
    if entry.kind() == EntryKind::Symlink {
        return follow_symlinks(files, entry);
    }
    Some(entry)
}

/// The Tag `source-is-missing`: a file that is built from some source, with no
/// source in the tree, in `debian/missing-sources/` or in the places lintian
/// looks.
pub(super) fn source_is_missing(non_free: bool, files: &FileTree, entry: &Entry) -> bool {
    let magic = prefix(files, entry);

    if is_python_byte_code(&magic) && !find_source(files, entry, non_free, &PYTHON) {
        return true;
    }
    if matches!(entry.file_type(), Some(FileType::Elf { .. }))
        && !find_source(files, entry, non_free, &ELF)
    {
        return true;
    }
    if is_macromedia_flash(&magic) && !find_source(files, entry, non_free, &FLASH_OBJECT) {
        return true;
    }
    if is_composite_document(&magic)
        && base_name(entry).to_lowercase().ends_with(".fla")
        && !find_source(files, entry, non_free, &FLASH_PROJECT)
    {
        return true;
    }
    if is_deploy_java(files, entry) {
        return !find_source(files, entry, non_free, &DEPLOY_JAVA_SOURCE);
    }
    if PREBUILT_JS_NAME.is_match(&name_bytes(entry)) {
        return !find_source(files, entry, non_free, &PREBUILT_JS_SOURCE);
    }
    let base = base_name(entry);
    let is_js = JS_SUFFIX.is_match(&base);
    let is_html = HTML_SUFFIX.is_match(&base);
    if !(is_js || is_html) || !has_very_long_line(files, entry) {
        return false;
    }
    if is_js {
        return !find_source(files, entry, non_free, &LONG_LINE_JS_SOURCE);
    }
    !find_source(files, entry, non_free, &HTML_SOURCE)
}
