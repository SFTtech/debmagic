use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsMimeCacheFile;

declare_rule! {
    /// A non-directory entry is a direct child of `usr/share/mime/`, such as
    /// the generated `mime.cache` or `types` cache files.
    ///
    /// Regular files, hardlinks and symlinks are checked. Directories are not
    /// reported. Extra is empty; the path is the Location. Udebs are checked
    /// too.
    PackageContainsMimeCacheFile,
    code = "LN0964",
    tag = "package-contains-mime-cache-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsMimeCacheFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter(|entry| entry.path().to_str().is_some_and(is_direct_child))
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

/// A name directly below `usr/share/mime/`, with no further `/`.
fn is_direct_child(name: &str) -> bool {
    name.strip_prefix("usr/share/mime/")
        .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("usr/share/mime/packages/foo.xml", TestFile::new("x")),
            data("usr/share/mime", TestFile::directory()),
            // A directory directly below is not a cache file.
            data("usr/share/mime/foo", TestFile::directory()),
            data("usr/share/mimex/types", TestFile::new("x")),
            data("usr/share/doc/mime/types", TestFile::new("x")),
        ];
        let fail = vec![
            data("usr/share/mime/types", TestFile::new("x")),
            data("usr/share/mime/mime.cache", TestFile::new("x")),
            data("usr/share/mime/types", TestFile::symlink("packages/types")),
            data("usr/share/mime/types", TestFile::new("x")).udeb(),
        ];
        BinaryPackageTester::new(PackageContainsMimeCacheFile, pass, fail).test_and_snapshot();
    }
}
