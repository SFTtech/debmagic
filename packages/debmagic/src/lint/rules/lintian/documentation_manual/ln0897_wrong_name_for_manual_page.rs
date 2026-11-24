use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind};
use crate::lint::rule::BinaryPackageRule;

pub struct WrongNameForManualPage;

declare_rule! {
    /// A manual page whose file name lacks a `*section*` part.
    ///
    /// Checked for every file, hardlink, or symlink under `usr/share/man/`
    /// in a `manN/` directory (or a locale's `manN/`). The name must have a
    /// component before the optional `gz` that starts with a digit, with at
    /// least one component before it: `foo.1.gz` and `foo.1p.gz` pass,
    /// `example.man.gz`, `simple.pod.gz`, and `README` fail. Pages directly in
    /// `usr/share/man/`, and pages in a directory without `manN`, are left to
    /// the odd-place Tag. Extra is empty. The Location is the file. Applies to
    /// deb, udeb, and ddeb.
    WrongNameForManualPage,
    code = "LN0897",
    tag = "wrong-name-for-manual-page",
    default_selected = true,
    experimental = false,
}

static SECTION_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"man\d/$").expect("valid regex"));

impl BinaryPackageRule for WrongNameForManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits: Vec<PathBuf> = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            files
                .entries()
                .iter()
                .filter(|entry| is_leaf(entry))
                .filter(|entry| in_section_directory(entry))
                .filter(|entry| has_wrong_name(entry))
                .map(|entry| entry.path().to_path_buf())
                .collect()
        };
        for path in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn is_leaf(entry: &Entry) -> bool {
    matches!(
        entry.kind(),
        EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
    )
}

/// The parent is `usr/share/man/<anything>/manN` (or `usr/share/man/manN`),
/// with the first character after `usr/share/man/` not whitespace.
fn in_section_directory(entry: &Entry) -> bool {
    let Some(parent) = entry.path().parent().and_then(Path::to_str) else {
        return false;
    };
    let Some(rest) = parent.strip_prefix("usr/share/man/") else {
        return false;
    };
    if rest.starts_with(char::is_whitespace) {
        return false;
    }
    SECTION_DIR.is_match(&format!("/{rest}/"))
}

/// Lintian: drop a trailing `gz`, then the component before it must exist
/// after the section is removed, and start with a digit.
fn has_wrong_name(entry: &Entry) -> bool {
    let Some(name) = entry.path().file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let mut pieces = dot_pieces(name);
    let Some(extension) = pieces.pop() else {
        return true;
    };
    if extension != "gz" {
        pieces.push(extension);
    }
    let section = pieces.pop();
    let Some(section) = section.filter(|_| !pieces.is_empty()) else {
        return true;
    };
    !section.starts_with(|character: char| character.is_ascii_digit())
}

/// Perl `split(/\./, $name)`: leading empties stay, trailing empties go.
fn dot_pieces(name: &str) -> Vec<&str> {
    let mut pieces: Vec<&str> = name.split('.').collect();
    while pieces.last().is_some_and(|piece| piece.is_empty()) {
        pieces.pop();
    }
    pieces
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn one(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new(".TH FOO 1\n"))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            one("usr/share/man/man1/foo.1.gz"),
            one("usr/share/man/man1/foo.bar.1.gz"),
            one("usr/share/man/man1/foo.1p.gz"),
            one("usr/share/man/de/man1/foo.1.gz"),
            one("usr/share/man/man1/foo.1"),
            // Not in a manN directory: odd-place, not this Tag.
            one("usr/share/man/zecho.gz"),
            one("usr/share/man/man-lintian/README.gz"),
            one("usr/share/doc/example.man.gz"),
        ];
        let fail = vec![
            one("usr/share/man/man1/example.man.gz"),
            one("usr/share/man/man6/simple.pod.gz"),
            one("usr/share/man/man1/README"),
            one("usr/share/man/man1/1.gz"),
            one("usr/share/man/de/man1/foo.gz"),
            BinaryPackageCase::from(vec![(
                "usr/share/man/man1/link.man.gz",
                TestFile::symlink("foo.1.gz"),
            )]),
        ];
        BinaryPackageTester::new(WrongNameForManualPage, pass, fail).test_and_snapshot();
    }
}
