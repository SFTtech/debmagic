use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::content_type::ContentType;
use crate::lint::context::SourceContext;
use crate::lint::data::lines;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::entry_bytes;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct LicenseProblemNonFreeImgLenna;

declare_rule! {
    /// A Patched file that is the Lenna test image, a picture cropped from a
    /// Playboy centerfold: distributable but not free. Reported at error
    /// severity, with an empty extra and the file's path as the Location.
    ///
    /// Only files whose content is an image, a Matlab v4 to v7.3 MAT-file, a
    /// PDF or a PostScript file are considered, as lintian's `file_type` test.
    /// A file whose MD5 is in the known-good list (`files/banned/lenna/whitelist`)
    /// is skipped. A file is reported when its base name contains `lena` or
    /// `lenna` as a word (`_` counts as a boundary), or when its MD5 is in the
    /// known-bad list (`files/banned/lenna/blacklist`). Both tables are vendored
    /// from lintian 2.144.0. Paths under `.pc/` are skipped.
    ///
    /// Differs from lintian: the content type is sniffed from the leading bytes
    /// (`ContentType`), not from `file(1)`. The gate covers the formats the
    /// recipes and the Lenna test images use; a format `file(1)` recognises and
    /// the sniffer does not is not checked.
    LicenseProblemNonFreeImgLenna,
    code = "LN1376",
    tag = "license-problem-non-free-img-lenna",
    default_selected = true,
    experimental = false,
}

/// lintian's `$self->LENNA_BLACKLIST` keys: the MD5 before the first `~~`.
static LENNA_BLACKLIST: LazyLock<HashSet<String>> = LazyLock::new(|| {
    lines(include_str!("../../../data/files/banned/lenna/blacklist"))
        .filter_map(|line| line.split("~~").next().map(|md5| md5.trim().to_string()))
        .collect()
});

/// lintian's `$self->LENNA_WHITELIST`: one MD5 per line.
static LENNA_WHITELIST: LazyLock<HashSet<String>> = LazyLock::new(|| {
    lines(include_str!("../../../data/files/banned/lenna/whitelist"))
        .map(str::to_string)
        .collect()
});

/// `/ ( \b | _ ) lenn?a ( \b | _ ) /ix` on the base name.
static LENNA_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\b|_)lenn?a(\b|_)").expect("valid regex"));

impl SourceRule for LicenseProblemNonFreeImgLenna {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        for path in lenna_files(files) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// The regular files and hardlinks of the tree (outside `.pc/`) that are a
/// Lenna image, in tree order.
fn lenna_files(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| !entry.path().starts_with(".pc"))
        .filter(|entry| {
            let Some(bytes) = entry_bytes(files, entry) else {
                return false;
            };
            let Some(content) = ContentType::sniff(&bytes) else {
                return false;
            };
            if !is_checked_type(content) {
                return false;
            }
            let Some(digest) = files.md5sum(entry) else {
                return false;
            };
            if LENNA_WHITELIST.contains(&digest) {
                return false;
            }
            let basename = entry
                .path()
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            LENNA_NAME.is_match(&basename) || LENNA_BLACKLIST.contains(&digest)
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

/// lintian's `file_type` test: an image, a Matlab MAT-file, a PDF or a
/// PostScript document.
fn is_checked_type(content: ContentType) -> bool {
    content.is_image()
        || matches!(
            content,
            ContentType::Matlab { .. } | ContentType::Pdf | ContentType::PostScript
        )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::lint::file_tree::Entry;
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest";

    fn tree(files: Vec<(&str, &[u8])>) -> FileTree {
        FileTree::from_entries(
            None,
            files
                .into_iter()
                .map(|(path, content)| {
                    Entry::regular_file(path, None, Some(content.to_vec().into()))
                })
                .collect(),
        )
    }

    #[test]
    fn reports_lenna_names_of_image_types_only() {
        let files = tree(vec![
            ("img/lena.png", PNG),
            ("img/Lenna_small.jpg", b"\xff\xd8\xffrest"),
            ("img/data-fake-lenna.pdf", b"%PDF-1.3\n"),
            ("img/data-fake-lenna.mat", b"MATLAB 5.0 MAT-file"),
            ("img/lenna.txt", b"plain text"),
            ("img/allenna.png", PNG),
            ("img/elena.png", PNG),
            (".pc/lena.png", PNG),
        ]);
        let paths: Vec<_> = lenna_files(&files);
        assert_eq!(
            paths,
            [
                PathBuf::from("img/Lenna_small.jpg"),
                PathBuf::from("img/data-fake-lenna.mat"),
                PathBuf::from("img/data-fake-lenna.pdf"),
                PathBuf::from("img/lena.png"),
            ]
        );
        assert!(
            !paths
                .iter()
                .any(|path| path == Path::new("img/allenna.png"))
        );
    }

    #[test]
    fn name_boundaries_follow_lintian_regex() {
        assert!(LENNA_NAME.is_match("lena.png"));
        assert!(LENNA_NAME.is_match("fake-lenna.png"));
        assert!(LENNA_NAME.is_match("lena_std.jpg"));
        assert!(LENNA_NAME.is_match("my_lena_crop.png"));
        assert!(!LENNA_NAME.is_match("allenna.png"));
        assert!(!LENNA_NAME.is_match("elena.png"));
    }

    #[test]
    fn vendored_tables_hold_md5_keys_only() {
        assert!(LENNA_WHITELIST.contains("0ea16a2c4b94f0c1a83d11278655ac9f"));
        assert_eq!(LENNA_WHITELIST.len(), 1);
        assert!(LENNA_BLACKLIST.contains("36ad1cdb7c7855fcc253062c767c5748"));
        assert_eq!(LENNA_BLACKLIST.len(), 9);
        assert!(LENNA_BLACKLIST.iter().all(|md5| md5.len() == 32));
    }

    #[test]
    fn source_tree() {
        let pass: Vec<TestCase> = vec![TestCase::from(vec![("README", TestFile::new("readme"))])];
        let fail = Vec::<TestCase>::new();
        SourceTreeTester::new(LicenseProblemNonFreeImgLenna, pass, fail).test();
    }
}
