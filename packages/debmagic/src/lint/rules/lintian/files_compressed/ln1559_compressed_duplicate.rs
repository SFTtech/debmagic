use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct CompressedDuplicate;

declare_rule! {
    /// A compressed file is installed next to an uncompressed file of the
    /// same name, for example `usr/share/doc/foo.gz` beside `usr/share/doc/foo`.
    ///
    /// Walks the Data files regular files and hardlinks (lintian's `is_file`)
    /// whose name ends in a compression extension from
    /// `files/compressed-file-extensions`, with a non-empty stem. Reported when
    /// the stem is also an entry of the Data files, of any kind, as in lintian.
    /// Extra is empty and the compressed file is the Location. Severity is info
    /// and the Tag is not selected by default, as in lintian.
    CompressedDuplicate,
    code = "LN1559",
    tag = "compressed-duplicate",
    default_selected = false,
    experimental = false,
}

static COMPRESSED_NAME: LazyLock<Regex> = LazyLock::new(|| {
    let extensions: Vec<String> = crate::lint::data::lines(include_str!(
        "../../../data/files/compressed-file-extensions"
    ))
    .map(regex::escape)
    .collect();
    Regex::new(&format!(r"^(.+)\.(?:{})$", extensions.join("|"))).expect("valid regex")
});

impl BinaryPackageRule for CompressedDuplicate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let duplicates: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| {
                let name = entry.path().to_string_lossy();
                COMPRESSED_NAME
                    .captures(&name)
                    .is_some_and(|captures| files.get(Path::new(&captures[1])).is_some())
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in duplicates {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    })
                    .with_severity(Severity::Info),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("usr/share/doc/foo.gz", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/foo.txt", TestFile::new("x")),
                ("usr/share/doc/foo.gz", TestFile::new("x")),
            ]),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/foo", TestFile::new("x")),
                ("usr/share/doc/foo.gz", TestFile::symlink("foo")),
            ]),
            BinaryPackageCase::from(vec![
                (".gz", TestFile::new("x")),
                ("usr/share/doc/foo.bar", TestFile::new("x")),
            ]),
            BinaryPackageCase::from(vec![
                ("foo.tar", TestFile::new("x")),
                ("foo", TestFile::new("x")),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![
                ("usr/share/doc/foo.txt", TestFile::new("x")),
                ("usr/share/doc/foo.txt.gz", TestFile::new("x")),
            ]),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/foo", TestFile::new("x")),
                ("usr/share/doc/foo.gz", TestFile::new("x")),
            ]),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/foo", TestFile::new("x")),
                ("usr/share/doc/foo.bz2", TestFile::new("x")),
            ]),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/foo", TestFile::new("x")),
                ("usr/share/doc/foo.Z", TestFile::new("x")),
            ]),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/foo.tar", TestFile::new("x")),
                ("usr/share/doc/foo.tar.xz", TestFile::new("x")),
            ]),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/foo", TestFile::directory()),
                ("usr/share/doc/foo.gz", TestFile::new("x")),
            ]),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/foo", TestFile::new("x")),
                ("usr/share/doc/foo.gz", TestFile::new("x").hardlink()),
            ]),
        ];
        BinaryPackageTester::new(CompressedDuplicate, pass, fail).test_and_snapshot();
    }
}
