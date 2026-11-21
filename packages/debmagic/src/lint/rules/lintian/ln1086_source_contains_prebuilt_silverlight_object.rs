use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct SourceContainsPrebuiltSilverlightObject;

declare_rule! {
    /// The Source tree or Source package contains a prebuilt Silverlight
    /// control: a zip archive whose name ends in `.xac` (case-insensitive).
    /// Such files come from non-free build tools and must be removed.
    ///
    /// Regular files and hardlinks are checked. The name test is a byte
    /// suffix match, as in lintian's `\.xac$`. The archive test sniffs the
    /// `PK\x03\x04` local-file header in the bytes, standing in for
    /// `file(1)`'s "Zip archive data"; `file` subtypes such as JAR or OOXML
    /// are not split out. Walks the packaging File tree (Source tree, or
    /// Source package Patched files). Quilt `.pc/` is filtered out;
    /// `debian/missing-sources/` is skipped. Extra is the Entry path, as in
    /// the oracle hint; the Location is the same path.
    SourceContainsPrebuiltSilverlightObject,
    code = "LN1086",
    tag = "source-contains-prebuilt-silverlight-object",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SourceContainsPrebuiltSilverlightObject {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        for path in silverlight_paths(files) {
            // The oracle hint is the bare path, not `[path]`, so the path is the extra.
            let extra = path.display().to_string();
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn silverlight_paths(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| {
            let path = entry.path();
            !path.starts_with(".pc")
                && !path.starts_with("debian/missing-sources")
                && has_xac_suffix(entry)
                && is_zip(files, entry)
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

fn has_xac_suffix(entry: &Entry) -> bool {
    entry
        .path()
        .as_os_str()
        .as_bytes()
        .to_ascii_lowercase()
        .ends_with(b".xac")
}

fn is_zip(files: &FileTree, entry: &Entry) -> bool {
    files
        .read(entry)
        .is_ok_and(|bytes| bytes.starts_with(b"PK\x03\x04"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn zip() -> TestFile {
        TestFile::from_bytes(b"PK\x03\x04\x14\x00rest-of-archive".to_vec())
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("ui/app.xac", TestFile::new("not a zip")),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("ui/app.zip", zip()),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (".pc/app.xac", zip()),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("debian/missing-sources/app.xac", zip()),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("ui/app.xac", TestFile::symlink("app.zip")),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("ui/app.xac", TestFile::directory()),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("ui/app.xac", zip()),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("fakesilverlight.XAC", zip()),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("ui/app.xac", zip().hardlink()),
            ],
        ];
        SourceTreeTester::new(SourceContainsPrebuiltSilverlightObject, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(vec![("README", TestFile::new("hello"))]),
            SourcePackageCase::from(vec![(".pc/app.xac", zip())]),
            SourcePackageCase::from(vec![("debian/missing-sources/app.xac", zip())])
                .orig(vec![("ui/app.xac", zip())]),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![("ui/app.xac", zip())]),
            SourcePackageCase::from(vec![("ui/app.XAC", zip())]),
        ];
        SourcePackageTester::new(SourceContainsPrebuiltSilverlightObject, pass, fail)
            .test_and_snapshot();
    }
}
