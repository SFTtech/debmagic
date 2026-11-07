use std::path::PathBuf;

use regex::Regex;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct IllegalVariable;

declare_rule! {
    /// A `debian/*install` or `debian/*links` file uses `DEB_BUILD_MULTIARCH`.
    ///
    /// Any regular file under `debian/` whose basename is `install`, `links`,
    /// or `<something>.install` / `<something>.links` is read whole. The word
    /// `DEB_BUILD_MULTIARCH` on any line is reported once per file. Extra is
    /// the variable name. Location is that file, without a line.
    IllegalVariable,
    code = "LN0369",
    tag = "illegal-variable",
    default_selected = true,
    experimental = false,
}

const VARIABLE: &str = "DEB_BUILD_MULTIARCH";

static WANTED_BASENAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.+\.)?(?:install|links)$").expect("valid regex"));

static VARIABLE_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bDEB_BUILD_MULTIARCH\b").expect("valid regex"));

impl SourceRule for IllegalVariable {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let paths = match ctx.files() {
            Some(files) => offending_files(files),
            None => return,
        };
        for path in paths {
            ctx.diagnostic(Diagnostic::error(VARIABLE).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn offending_files(files: &FileTree) -> Vec<PathBuf> {
    files
        .entries()
        .iter()
        .filter(|entry| wanted(entry))
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            let text = String::from_utf8_lossy(&bytes);
            VARIABLE_WORD
                .is_match(&text)
                .then(|| entry.path().to_path_buf())
        })
        .collect()
}

fn wanted(entry: &Entry) -> bool {
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return false;
    }
    let path = entry.path();
    if !path.starts_with("debian") {
        return false;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| WANTED_BASENAME.is_match(name))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(path: &'static str, body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new("Source: example\n")),
            (path, TestFile::new(body)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("debian/install", "usr/bin/example\n"),
            tree("debian/example.links", "usr/share/a usr/share/b\n"),
            // Other basenames are not read.
            tree("debian/xinstall", "DEB_BUILD_MULTIARCH\n"),
            tree("debian/rules", "DEB_BUILD_MULTIARCH\n"),
            // The variable must be a whole word.
            tree("debian/install", "usr/lib/DEB_BUILD_MULTIARCHX/\n"),
        ];
        let fail = vec![
            tree("debian/install", "usr/lib/${DEB_BUILD_MULTIARCH}/example\n"),
            tree(
                "debian/links",
                "usr/lib/DEB_BUILD_MULTIARCH/a usr/share/b\n",
            ),
            tree(
                "debian/example.install",
                "usr/lib/DEB_BUILD_MULTIARCH/\nusr/lib/DEB_BUILD_MULTIARCH/x\n",
            ),
        ];
        SourceTreeTester::new(IllegalVariable, pass, fail).test_and_snapshot();
    }
}
