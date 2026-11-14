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
static DOCTREE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.doctree(?:\.gz)?$").expect("valid regex"));
static PY_HEADER_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/include/python3\.[0-7]/$").expect("valid regex"));
static PY_TESTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/python[^/]+/(?:dist|site)-packages/test_.+\.py$").expect("valid regex")
});

const SKEL_ALLOWED: &[&str] = &[".bashrc", ".bash_logout", ".kshrc", ".mkshrc", ".profile"];

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
    Star,
    EtcSkel,
    Hal,
    IconCache,
    PythonCoverage,
    PythonDoctree,
    PythonHeader,
    PythonTests,
    Eslint,
    NpmIgnore,
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
    if name.ends_with("/*") {
        hits.push(NameHit::Star);
    }
    let basename = path
        .file_name()
        .and_then(|base| base.to_str())
        .unwrap_or("");
    if dirname.starts_with("etc/skel/") && !SKEL_ALLOWED.contains(&basename) {
        hits.push(NameHit::EtcSkel);
    }
    if dirname.starts_with("usr/share/hal/") {
        hits.push(NameHit::Hal);
    }
    if raw == "usr/share/icons/hicolor/icon-theme.cache" {
        hits.push(NameHit::IconCache);
    }
    if basename == ".coverage" {
        hits.push(NameHit::PythonCoverage);
    }
    if DOCTREE.is_match(basename) {
        hits.push(NameHit::PythonDoctree);
    }
    if PY_HEADER_DIR.is_match(&dirname) && raw.ends_with(".h") {
        hits.push(NameHit::PythonHeader);
    }
    if PY_TESTS.is_match(raw) {
        hits.push(NameHit::PythonTests);
    }
    if basename.starts_with(".eslintrc") {
        hits.push(NameHit::Eslint);
    }
    if basename == ".npmignore" {
        hits.push(NameHit::NpmIgnore);
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
            [NameHit::Wildcard, NameHit::Star]
        );
        assert_eq!(
            name_hits(Path::new("usr/share/foo/*/nested"), file),
            [NameHit::Wildcard]
        );
        assert_eq!(name_hits(Path::new("*"), file), [NameHit::Wildcard]);
        assert_eq!(
            name_hits(Path::new("etc/skel/.lintianrc"), file),
            [NameHit::EtcSkel]
        );
        assert!(name_hits(Path::new("etc/skel/.bashrc"), file).is_empty());
        assert!(name_hits(Path::new("etc/skel/.mkshrc"), file).is_empty());
        assert!(name_hits(Path::new("etc/skel"), dir).is_empty());
        assert_eq!(
            name_hits(Path::new("usr/share/hal/foo.fdi"), file),
            [NameHit::Hal]
        );
        assert_eq!(
            name_hits(Path::new("usr/share/icons/hicolor/icon-theme.cache"), file),
            [NameHit::IconCache]
        );
        assert_eq!(
            name_hits(Path::new("usr/share/pkg/.coverage"), file),
            [NameHit::PythonCoverage]
        );
        assert_eq!(
            name_hits(Path::new("usr/share/foo/foo.doctree"), file),
            [NameHit::PythonDoctree]
        );
        assert_eq!(
            name_hits(Path::new("usr/share/foo/foo.doctree.gz"), file),
            [NameHit::PythonDoctree]
        );
        assert!(name_hits(Path::new("usr/share/foo/foo.doctree.bak"), file).is_empty());
        assert_eq!(
            name_hits(Path::new("usr/include/python3.7/bad.h"), file),
            [NameHit::PythonHeader]
        );
        assert!(name_hits(Path::new("usr/include/python3.8/bad.h"), file).is_empty());
        assert!(name_hits(Path::new("usr/include/python3.7/sub/bad.h"), file).is_empty());
        assert_eq!(
            name_hits(Path::new("usr/lib/python3/dist-packages/test_foo.py"), file),
            [NameHit::PythonTests]
        );
        assert!(
            name_hits(
                Path::new("usr/lib/python3/dist-packages/pkg/test_foo.py"),
                file
            )
            .is_empty()
        );
        assert_eq!(
            name_hits(Path::new("usr/share/doc/pkg/.eslintrc.js"), file),
            [NameHit::Eslint]
        );
        assert_eq!(
            name_hits(Path::new("usr/share/doc/pkg/.npmignore"), file),
            [NameHit::NpmIgnore]
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
