use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind};
use crate::lint::rule::BinaryPackageRule;

pub struct OddPlaceForManualPage;

declare_rule! {
    /// A manual page installed where its section does not belong.
    ///
    /// Checked for every file, hardlink, or symlink (not directories) whose
    /// parent is under `usr/share/man/`. Three shapes are reported, each once
    /// per entry: a file directly in `usr/share/man/`; a directory below
    /// `usr/share/man/` whose last component is not `manN`; and a page whose
    /// name section starts with a digit that differs from the digit of its
    /// `manN` directory (`foo.2.gz` in `man1`). The name section is the
    /// component before `.gz`, so `foo.1p.gz` has section `1`. Extra is empty.
    /// The Location is the entry. Applies to deb, udeb, and ddeb.
    OddPlaceForManualPage,
    code = "LN0898",
    tag = "odd-place-for-manual-page",
    default_selected = true,
    experimental = false,
}

static SECTION_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"man(\d)/$").expect("valid regex"));

impl BinaryPackageRule for OddPlaceForManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits: Vec<PathBuf> = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            files
                .entries()
                .iter()
                .filter(|entry| is_leaf(entry))
                .filter(|entry| is_odd_place(entry))
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

fn is_odd_place(entry: &Entry) -> bool {
    let path = entry.path();
    let Some(parent) = path.parent().and_then(Path::to_str) else {
        return false;
    };
    if parent == "usr/share/man" {
        return path.file_name().is_some();
    }
    let Some(rest) = parent.strip_prefix("usr/share/man") else {
        return false;
    };
    // `usr/share/man(/\S+)`: a slash, then a non-space character.
    if !rest.starts_with('/') || rest[1..].starts_with(char::is_whitespace) || rest.len() < 2 {
        return false;
    }
    let subdir = format!("{rest}/");
    let Some(captures) = SECTION_DIR.captures(&subdir) else {
        return true;
    };
    let directory_section = captures[1].chars().next();
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let mut pieces = dot_pieces(name);
    let Some(extension) = pieces.pop() else {
        return false;
    };
    if extension != "gz" {
        pieces.push(extension);
    }
    let Some(section) = pieces.pop() else {
        return false;
    };
    if pieces.is_empty() {
        return false;
    }
    match (section.chars().next(), directory_section) {
        (Some(name_digit), Some(dir_digit)) if name_digit.is_ascii_digit() => {
            name_digit != dir_digit
        }
        _ => false,
    }
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
            one("usr/share/man/de/man1/foo.1.gz"),
            one("usr/share/man/man1/foo.1p.gz"),
            one("usr/share/man/man3/foo.3.gz"),
            // No section in the name: the wrong-name Tag, not this one.
            one("usr/share/man/man1/README.gz"),
            // Symlinks follow the same rules.
            BinaryPackageCase::from(vec![(
                "usr/share/man/man2/link.2.gz",
                TestFile::symlink("foo.2.gz"),
            )]),
            // A directory is not a page.
            BinaryPackageCase::from(vec![("usr/share/man/man1/sub", TestFile::directory())]),
            one("usr/share/doc/foo.1.gz"),
        ];
        let fail = vec![
            one("usr/share/man/zecho.1.gz"),
            one("usr/share/man/man-lintian/zecho.1.gz"),
            one("usr/share/man/man1/foo.2.gz"),
            one("usr/share/man/man1/foo.2"),
            one("usr/share/man/de/man1/foo.5.gz"),
            BinaryPackageCase::from(vec![(
                "usr/share/man/man1/link.5.gz",
                TestFile::symlink("foo.1.gz"),
            )]),
        ];
        BinaryPackageTester::new(OddPlaceForManualPage, pass, fail).test_and_snapshot();
    }
}
