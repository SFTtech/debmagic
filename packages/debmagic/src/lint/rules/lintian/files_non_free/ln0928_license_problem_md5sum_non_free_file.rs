use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::data::lines;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct LicenseProblemMd5sumNonFreeFile;

declare_rule! {
    /// A Patched file is a known file that may not be in main or contrib.
    ///
    /// The MD5 of each regular file or hardlink of the Patched files (paths
    /// under `.pc/` skipped) is looked up in lintian's `cruft/non-free-files`
    /// table, vendored as `data/files/non-free-files`. A hit is reported with
    /// the extra `usual name is <name>. <reason> See also <link>.`, the
    /// fields of the table entry, and the path as the Location. Skipped when the
    /// Source package's `Section` (the `debian/control` source paragraph,
    /// `main` when absent) starts with `non-free/`, `non-free-firmware/`,
    /// `restricted/` or `multiverse/`, as lintian's `is_non_free`.
    LicenseProblemMd5sumNonFreeFile,
    code = "LN0928",
    tag = "license-problem-md5sum-non-free-file",
    default_selected = true,
    experimental = false,
}

/// One entry of the table: the usual name, the reason, and the reference.
#[derive(Debug, PartialEq, Eq)]
struct Known {
    name: String,
    reason: String,
    link: String,
}

static NON_FREE_FILES: LazyLock<HashMap<String, Known>> =
    LazyLock::new(|| parse(include_str!("../../../data/files/non-free-files")));

static NON_FREE_SECTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:non-free|non-free-firmware|restricted|multiverse)/").expect("valid regex")
});

impl SourceRule for LicenseProblemMd5sumNonFreeFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let section = ctx
            .debian_control()
            .and_then(|control| control.source().unfolded_value("Section"))
            .filter(|section| !section.is_empty())
            .unwrap_or_else(|| "main".to_string());
        if NON_FREE_SECTION.is_match(&section) {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let hits = known_files(files, &NON_FREE_FILES);
        for (path, known) in hits {
            let extra = format!(
                "usual name is {}. {} See also {}.",
                known.name, known.reason, known.link
            );
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Parses the `md5 ~~ sha1 ~~ sha256 ~~ name ~~ reason ~~ link` lines of
/// lintian's table. A line without all six fields is skipped.
fn parse(text: &str) -> HashMap<String, Known> {
    lines(text)
        .filter_map(|line| {
            let fields: Vec<&str> = line.splitn(6, "~~").map(str::trim).collect();
            let [md5, _sha1, _sha256, name, reason, link] = fields.as_slice() else {
                return None;
            };
            Some((
                md5.to_ascii_lowercase(),
                Known {
                    name: name.to_string(),
                    reason: reason.to_string(),
                    link: link.to_string(),
                },
            ))
        })
        .collect()
}

/// The regular files and hardlinks of the tree (outside `.pc/`) whose MD5 is
/// in the table, with the table entry each one matches.
fn known_files<'table>(
    files: &FileTree,
    table: &'table HashMap<String, Known>,
) -> Vec<(std::path::PathBuf, &'table Known)> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| !entry.path().starts_with(".pc"))
        .filter_map(|entry| {
            let digest = files.md5sum(entry)?;
            table
                .get(&digest)
                .map(|known| (entry.path().to_path_buf(), known))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::lint::file_tree::Entry;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, TestFile};

    use crate::lint::digest::{DigestAlgorithm, hex_digest};

    use super::*;

    fn md5_hex(bytes: &[u8]) -> String {
        hex_digest(DigestAlgorithm::Md5, bytes)
    }

    #[test]
    fn parses_the_table_fields() {
        let line = "d41d8cd98f00b204e9800998ecf8427e ~~ sha1 ~~ sha256 ~~ name.bin ~~ Reason text ~~ https://example.org";
        let table = parse(line);
        assert_eq!(
            table.get("d41d8cd98f00b204e9800998ecf8427e"),
            Some(&Known {
                name: "name.bin".to_string(),
                reason: "Reason text".to_string(),
                link: "https://example.org".to_string(),
            })
        );
        assert!(parse("# comment\nshort ~~ line").is_empty());
    }

    #[test]
    fn md5_matches_known_digest() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn matches_table_entries_outside_quilt_control() {
        let table = HashMap::from([(
            md5_hex(b"non-free content"),
            Known {
                name: "name.bin".to_string(),
                reason: "Reason.".to_string(),
                link: "https://example.org".to_string(),
            },
        )]);
        let files = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file("README", None, Some(b"readme".to_vec().into())),
                Entry::regular_file(
                    "docs/data.bin",
                    None,
                    Some(b"non-free content".to_vec().into()),
                ),
                Entry::regular_file(
                    ".pc/docs/data.bin",
                    None,
                    Some(b"non-free content".to_vec().into()),
                ),
            ],
        );
        let hits = known_files(&files, &table);
        let paths: Vec<_> = hits.iter().map(|(path, _)| path.as_path()).collect();
        assert_eq!(paths, [Path::new("docs/data.bin")]);
    }

    #[test]
    fn source() {
        // The vendored table does not list these fixture contents.
        let pass = vec![
            SourcePackageCase::from(vec![("README", TestFile::new("readme"))]),
            SourcePackageCase::from(vec![("README", TestFile::new("readme"))])
                .orig(vec![("README", TestFile::new("readme"))]),
        ];
        let fail = Vec::<SourcePackageCase>::new();
        SourcePackageTester::new(LicenseProblemMd5sumNonFreeFile, pass, fail).test();
    }
}
