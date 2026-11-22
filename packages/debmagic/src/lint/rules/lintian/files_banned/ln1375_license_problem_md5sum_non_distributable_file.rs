use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::data::lines;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct LicenseProblemMd5sumNonDistributableFile;

declare_rule! {
    /// A Patched file is a file that may not be distributed even in the non-free
    /// archive. The MD5 of each regular file or hardlink of the Patched files
    /// (paths under `.pc/` skipped) is looked up in lintian's
    /// `cruft/non-distributable-files` table, vendored as
    /// `data/cruft/non-distributable-files`. A hit is reported with the extra
    /// `usual name is <name>. <reason> See also <link>.` (the fields of the table
    /// entry) and the path as the Location, at error severity.
    ///
    /// Differs from lintian: the MD5 is computed by the lint run and shared
    /// with the other Rules (see `FileTree::md5sum`), and the table is read here
    /// with the same field split as lintian's `md5sum-based` loader.
    LicenseProblemMd5sumNonDistributableFile,
    code = "LN1375",
    tag = "license-problem-md5sum-non-distributable-file",
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

static NON_DISTRIBUTABLE_FILES: LazyLock<HashMap<String, Known>> =
    LazyLock::new(|| parse(include_str!("../../../data/cruft/non-distributable-files")));

impl SourceRule for LicenseProblemMd5sumNonDistributableFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        for (path, known) in known_files(files, &NON_DISTRIBUTABLE_FILES) {
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
/// lintian's table. A line without all six fields is skipped; a later line
/// with the same MD5 replaces an earlier one, as lintian's loader does.
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
/// in the table, with the table entry each one matches, in tree order.
fn known_files<'table>(
    files: &FileTree,
    table: &'table HashMap<String, Known>,
) -> Vec<(PathBuf, &'table Known)> {
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

    use crate::lint::digest::{DigestAlgorithm, hex_digest};
    use crate::lint::file_tree::Entry;
    use crate::lint::tester::{
        SourcePackageCase, SourcePackageTester, SourceTreeTester, TestCase, TestFile,
    };

    use super::*;

    const CONTENT: &[u8] = b"non-distributable content";

    fn table_for(content: &[u8]) -> HashMap<String, Known> {
        parse(&format!(
            "{} ~~ sha1 ~~ sha256 ~~ data.bin ~~ Reason text. ~~ https://example.org",
            hex_digest(DigestAlgorithm::Md5, content)
        ))
    }

    #[test]
    fn parses_the_table_fields() {
        let table = parse(
            "# comment\n\
             D41D8CD98F00B204E9800998ECF8427E ~~ sha1 ~~ sha256 ~~ name.bin ~~ Reason text ~~ https://example.org\n\
             short ~~ line\n",
        );
        assert_eq!(
            table.get("d41d8cd98f00b204e9800998ecf8427e"),
            Some(&Known {
                name: "name.bin".to_string(),
                reason: "Reason text".to_string(),
                link: "https://example.org".to_string(),
            })
        );
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn matches_table_entries_outside_quilt_control() {
        let table = table_for(CONTENT);
        let files = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file("README", None, Some(b"readme".to_vec())),
                Entry::regular_file("docs/data.bin", None, Some(CONTENT.to_vec())),
                Entry::hardlink("docs/copy.bin", None, Some(CONTENT.to_vec())),
                Entry::regular_file(".pc/docs/data.bin", None, Some(CONTENT.to_vec())),
            ],
        );
        let hits = known_files(&files, &table);
        let paths: Vec<_> = hits.iter().map(|(path, _)| path.as_path()).collect();
        assert_eq!(
            paths,
            [Path::new("docs/copy.bin"), Path::new("docs/data.bin")]
        );
        assert_eq!(hits[0].1.name, "data.bin");
    }

    #[test]
    fn source_tree() {
        let pass: Vec<TestCase> = vec![TestCase::from(vec![("README", TestFile::new("readme"))])];
        let fail = Vec::<TestCase>::new();
        SourceTreeTester::new(LicenseProblemMd5sumNonDistributableFile, pass, fail).test();
    }

    #[test]
    fn source_package() {
        // The vendored table does not list these fixture contents.
        let pass = vec![SourcePackageCase::from(vec![(
            "README",
            TestFile::new("readme"),
        )])];
        let fail = Vec::<SourcePackageCase>::new();
        SourcePackageTester::new(LicenseProblemMd5sumNonDistributableFile, pass, fail).test();
    }
}
