use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct SourceContainsCmakeCacheFile;

declare_rule! {
    /// The Source tree or Source package contains a CMake cache file.
    ///
    /// Regular files named `CMakeCache.txt` anywhere in the packaging File
    /// tree (Source tree, or Source package Patched files). Directories and
    /// symlinks are not flagged. Extra is empty; the path is the Location.
    SourceContainsCmakeCacheFile,
    code = "LN0066",
    tag = "source-contains-cmake-cache-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SourceContainsCmakeCacheFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile])
            .into_iter()
            .filter(|entry| {
                entry
                    .path()
                    .file_name()
                    .is_some_and(|name| name == "CMakeCache.txt")
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
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
            tree(&[("CMakeCache.txt.bak", TestFile::new("x"))]),
            tree(&[("CMakeCache.txt", TestFile::symlink("x"))]),
            tree(&[("CMakeCache.txt", TestFile::directory())]),
        ];
        let fail = vec![
            tree(&[("CMakeCache.txt", TestFile::new("x"))]),
            tree(&[("debian/CMakeCache.txt", TestFile::new("x"))]),
            tree(&[
                ("src/CMakeCache.txt", TestFile::new("x")),
                ("debian/CMakeCache.txt", TestFile::new("x")),
            ]),
        ];
        SourceTreeTester::new(SourceContainsCmakeCacheFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(&[]))];
        let fail = vec![SourcePackageCase::from(tree(&[(
            "CMakeCache.txt",
            TestFile::new("x"),
        )]))];
        SourcePackageTester::new(SourceContainsCmakeCacheFile, pass, fail).test_and_snapshot();
    }
}
