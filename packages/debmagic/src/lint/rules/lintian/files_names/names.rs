//! Path predicates from lintian's `files/names` `visit_installed_files`.
//!
//! Directory names are given the trailing slash lintian stores on directory
//! index entries (`dirname` of a child). Matching uses that form.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::EntryKind;

static PYTHON_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/python[^/]+/(?:dist|site)-packages/").expect("valid regex")
});
static DOT_SEGMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/[.][^/]+/").expect("valid regex"));
static HYPOTHESIS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/[.]hypothesis/examples/").expect("valid regex"));
static SASS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/[.]sass-cache/").expect("valid regex"));
static GLIB_SCHEMA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/share/glib-[^/]+/schemas/gschemas\.compiled$").expect("valid regex")
});

const PATH_DIRECTORIES: &[&str] = &["bin/", "sbin/", "usr/bin/", "usr/sbin/", "usr/games/"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NameHit {
    Whitespace,
    Wildcard,
    Hyphen,
    PythonDot,
    Hypothesis,
    Sass,
    PathNotAscii,
    GlibSchema,
}

pub(super) fn name_hits(path: &Path, kind: EntryKind) -> Vec<NameHit> {
    let Some(raw) = path.to_str() else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    if raw.chars().last().is_some_and(char::is_whitespace) {
        hits.push(NameHit::Whitespace);
    }
    if raw.contains(['*', '?']) {
        hits.push(NameHit::Wildcard);
    }
    if raw.ends_with("/-") {
        hits.push(NameHit::Hyphen);
    }
    if GLIB_SCHEMA.is_match(raw) {
        hits.push(NameHit::GlibSchema);
    }
    let name = lintian_name(raw, kind);
    let dirname = lintian_dirname(&name);
    if PYTHON_DIR.is_match(&dirname) && DOT_SEGMENT.is_match(&name) {
        hits.push(NameHit::PythonDot);
    }
    if HYPOTHESIS.is_match(&dirname) {
        hits.push(NameHit::Hypothesis);
    }
    if SASS.is_match(&name) {
        hits.push(NameHit::Sass);
    }
    if PATH_DIRECTORIES.contains(&dirname.as_str())
        && path
            .file_name()
            .is_some_and(|base| base.as_encoded_bytes().iter().any(|byte| !byte.is_ascii()))
    {
        hits.push(NameHit::PathNotAscii);
    }
    hits
}

/// Directory index names end in `/`. Files keep the path as stored.
fn lintian_name(path: &str, kind: EntryKind) -> String {
    if kind == EntryKind::Directory && !path.ends_with('/') {
        format!("{path}/")
    } else {
        path.to_string()
    }
}

/// `^(.+/)?(?:[^/]+/?)$` from `Lintian::Index::Item`, including the slash.
fn lintian_dirname(name: &str) -> String {
    static DIRNAME: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?:(.+)/)?(?:[^/]+/?)$").expect("valid regex"));
    DIRNAME
        .captures(name)
        .and_then(|caps| caps.get(1))
        .map(|parent| format!("{}/", parent.as_str()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirname_matches_the_check() {
        assert_eq!(
            lintian_dirname("usr/lib/python3/dist-packages/foo/.directory/bar"),
            "usr/lib/python3/dist-packages/foo/.directory/"
        );
        assert_eq!(
            lintian_dirname("usr/lib/python3/dist-packages/foo/.directory/"),
            "usr/lib/python3/dist-packages/foo/"
        );
        assert_eq!(lintian_dirname("usr/bin/tool"), "usr/bin/");
        assert_eq!(lintian_dirname("-"), "");
    }

    #[test]
    fn recipe_paths() {
        let file = EntryKind::RegularFile;
        let dir = EntryKind::Directory;
        assert_eq!(
            name_hits(Path::new("usr/share/foo/ws "), file),
            [NameHit::Whitespace]
        );
        assert_eq!(
            name_hits(Path::new("usr/share/foo/*"), file),
            [NameHit::Wildcard]
        );
        assert_eq!(
            name_hits(
                Path::new("usr/share/files-wildcard-characters/question-mark?"),
                file
            ),
            [NameHit::Wildcard]
        );
        assert_eq!(
            name_hits(Path::new("usr/share/foo/-"), file),
            [NameHit::Hyphen]
        );
        assert_eq!(
            name_hits(
                Path::new("usr/share/glib-2.0/schemas/gschemas.compiled"),
                file
            ),
            [NameHit::GlibSchema]
        );
        assert!(name_hits(Path::new("usr/share/foo/gschemas.compiled"), file).is_empty());
        assert_eq!(
            name_hits(
                Path::new("usr/lib/python3/dist-packages/foo/.directory/foo"),
                file
            ),
            [NameHit::PythonDot]
        );
        assert_eq!(
            name_hits(
                Path::new("usr/lib/python3/dist-packages/foo/.directory"),
                dir
            ),
            [NameHit::PythonDot]
        );
        assert!(name_hits(Path::new("usr/lib/python3/dist-packages/foo.py"), file).is_empty());
        assert_eq!(
            name_hits(
                Path::new(
                    "usr/share/doc/python-hypothesis-example/.hypothesis/examples/show/nothing"
                ),
                file
            ),
            [NameHit::Hypothesis]
        );
        assert_eq!(
            name_hits(
                Path::new("usr/share/doc/python-hypothesis-example/.hypothesis/examples/show"),
                dir
            ),
            [NameHit::Hypothesis]
        );
        assert_eq!(
            name_hits(
                Path::new("usr/share/doc/sass-cache-directory/.sass-cache/nothing"),
                file
            ),
            [NameHit::Sass]
        );
        assert_eq!(
            name_hits(
                Path::new("usr/share/doc/sass-cache-directory/.sass-cache"),
                dir
            ),
            [NameHit::Sass]
        );
        assert_eq!(
            name_hits(Path::new("usr/bin/鳥の詩"), file),
            [NameHit::PathNotAscii]
        );
        assert!(name_hits(Path::new("usr/share/foo/鳥"), file).is_empty());
        assert!(name_hits(Path::new("usr/bin/tool"), file).is_empty());
    }
}
