use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsMimeinfoCacheFile;

declare_rule! {
    /// An installed file is `usr/share/applications/mimeinfo.cache`, possibly
    /// gzipped.
    ///
    /// Any installed entry that is not a directory is checked, so a symlink
    /// or hardlink with that name is reported too. Extra is empty; the path
    /// is the Location. Udebs are checked too. Lintian's pattern leaves the
    /// dot unescaped, so `mimeinfoXcache` also matches; this Rule keeps that.
    PackageContainsMimeinfoCacheFile,
    code = "LN1133",
    tag = "package-contains-mimeinfo.cache-file",
    default_selected = true,
    experimental = false,
}

static MIMEINFO_CACHE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/share/applications/mimeinfo.cache(?:\.gz)?$").expect("valid regex")
});

impl BinaryPackageRule for PackageContainsMimeinfoCacheFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|name| MIMEINFO_CACHE.is_match(name))
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
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data(
                "usr/share/applications/mimeinfo.cache",
                TestFile::directory(),
            ),
            data(
                "usr/share/applications/mimeinfo.cache.gz",
                TestFile::directory(),
            ),
            data(
                "usr/share/applications/foo/mimeinfo.cache",
                TestFile::new("x"),
            ),
            data(
                "usr/share/applications/mimeinfo.cache.bz2",
                TestFile::new("x"),
            ),
            data(
                "usr/share/applications/mimeinfo.cache.old",
                TestFile::new("x"),
            ),
            data("usr/share/mimeinfo.cache", TestFile::new("x")),
            data(
                "usr/share/applications/mimeinfo.cache.gz.x",
                TestFile::new("x"),
            ),
        ];
        let fail = vec![
            data("usr/share/applications/mimeinfo.cache", TestFile::new("x")),
            data(
                "usr/share/applications/mimeinfo.cache.gz",
                TestFile::new("x"),
            ),
            data(
                "usr/share/applications/mimeinfo.cache",
                TestFile::symlink("foo"),
            ),
            data(
                "usr/share/applications/mimeinfo.cache",
                TestFile::new("x").hardlink(),
            ),
            data("usr/share/applications/mimeinfo.cache", TestFile::new("x")).udeb(),
            // The dot is unescaped in lintian's pattern.
            data("usr/share/applications/mimeinfoXcache", TestFile::new("x")),
        ];
        BinaryPackageTester::new(PackageContainsMimeinfoCacheFile, pass, fail).test_and_snapshot();
    }
}
