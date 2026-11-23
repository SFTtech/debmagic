use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct DebianFilesListInSource;

declare_rule! {
    /// The packaging tree leaves a non-empty `debian/files` behind.
    ///
    /// Walks the packaging File tree (Source tree on disk, or Source package
    /// Patched files) and reports every file named exactly `debian/files`
    /// with at least one byte. The file is the dpkg-genchanges list of `.deb`
    /// files of one build; left in the source it breaks the autobuilders for
    /// another architecture. An empty `debian/files` passes. Only the
    /// top-level `.pc/` (quilt) directory is skipped, as lintian does for
    /// patched files; a `debian/files` under any other directory is not
    /// reported. Regular files and hardlinks count; symlinks, directories and
    /// special files do not. Extra is empty; the path is the Location.
    DebianFilesListInSource,
    code = "LN1773",
    tag = "debian-files-list-in-source",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianFilesListInSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let mut found = Vec::new();
        for entry in files.entries() {
            let path = entry.path();
            if !entry.is_file() || path.starts_with(".pc") || path != Path::new("debian/files") {
                continue;
            }
            // Only the first byte matters: non-empty means size > 0.
            let non_empty = files
                .read_prefix(entry, 1)
                .is_ok_and(|prefix| !prefix.is_empty());
            if non_empty {
                found.push(path.to_path_buf());
            }
        }
        for path in found {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
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

    fn tree(extra: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![("debian/control".to_string(), TestFile::from(CONTROL))];
        files.extend(
            extra
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone())),
        );
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            // An empty `debian/files` is fine.
            tree(&[("debian/files", TestFile::new(""))]),
            // Only the exact path under the packaging directory.
            tree(&[("files", TestFile::new("x\n"))]),
            tree(&[("debian/files.bak", TestFile::new("x\n"))]),
            tree(&[("debian/sub/files", TestFile::new("x\n"))]),
            tree(&[("src/debian/files", TestFile::new("x\n"))]),
            // Quilt's top-level `.pc/` is skipped.
            tree(&[(".pc/debian/files", TestFile::new("x\n"))]),
            // Directories, symlinks and special files are not files.
            tree(&[("debian/files", TestFile::directory())]),
            tree(&[("debian/files", TestFile::symlink("elsewhere"))]),
            tree(&[("debian/files", TestFile::fifo())]),
        ];
        let fail = vec![
            tree(&[("debian/files", TestFile::new("example_1.0-1_amd64.deb\n"))]),
            // A single byte is enough.
            tree(&[("debian/files", TestFile::new("\n"))]),
            // A hardlink with content counts as a file.
            tree(&[("debian/files", TestFile::new("shared\n").hardlink())]),
        ];
        SourceTreeTester::new(DebianFilesListInSource, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(vec![("debian/control", TestFile::from(CONTROL))]),
            SourcePackageCase::from(vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("debian/files", TestFile::new("")),
            ]),
        ];
        let fail = vec![SourcePackageCase::from(vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("debian/files", TestFile::new("example_1.0-1_amd64.deb\n")),
        ])];
        SourcePackageTester::new(DebianFilesListInSource, pass, fail).test_and_snapshot();
    }
}
