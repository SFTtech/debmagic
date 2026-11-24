use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsMimeFileOutsidePackageDir;

declare_rule! {
    /// An installed path below `usr/share/mime/` is not a direct child of it
    /// and is not under `usr/share/mime/packages/`.
    ///
    /// Walks every Data files entry. A directory is matched with its trailing
    /// slash. Direct children are the cache files of another Tag and are not
    /// reported. Extra is empty; the path is the Location. Udebs are checked
    /// too.
    PackageContainsMimeFileOutsidePackageDir,
    code = "LN0821",
    tag = "package-contains-mime-file-outside-package-dir",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsMimeFileOutsidePackageDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter_map(lintian_name)
            .filter(|name| is_outside_package_dir(name))
            .map(PathBuf::from)
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

/// The path of an Entry as lintian names an installed item: a directory has a
/// trailing slash. Non-UTF-8 paths are skipped.
fn lintian_name(entry: &Entry) -> Option<String> {
    let path = entry.path().to_str()?;
    Some(if matches!(entry.kind(), EntryKind::Directory) {
        format!("{path}/")
    } else {
        path.to_string()
    })
}

/// Below `usr/share/mime/`, and neither a direct child nor under `packages/`.
fn is_outside_package_dir(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("usr/share/mime/") else {
        return false;
    };
    if rest.is_empty() || !rest.contains('/') {
        return false;
    }
    !rest.starts_with("packages/")
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
            data("usr/share/mime/packages/sub/foo.xml", TestFile::new("x")),
            data("usr/share/mime/packages", TestFile::directory()),
            data("usr/share/mime", TestFile::directory()),
            // Direct children are cache files, another Tag.
            data("usr/share/mime/mime.cache", TestFile::new("x")),
            data("usr/share/mime/foo", TestFile::new("x")),
            // Other directories are not under usr/share/mime/.
            data("usr/share/mimex/foo/bar", TestFile::new("x")),
            data("usr/share/doc/mime/foo/bar", TestFile::new("x")),
        ];
        let fail = vec![
            data("usr/share/mime/foo/bar", TestFile::new("x")),
            data("usr/share/mime/foo", TestFile::directory()),
            data("usr/share/mime/packagesx/foo.xml", TestFile::new("x")),
            data("usr/share/mime/foo/bar", TestFile::symlink("baz")),
            data("usr/share/mime/foo/bar", TestFile::new("x")).udeb(),
        ];
        BinaryPackageTester::new(PackageContainsMimeFileOutsidePackageDir, pass, fail)
            .test_and_snapshot();
    }
}
