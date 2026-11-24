use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::{emit, visit};

pub struct PackageContainsInfoDirFile;

declare_rule! {
    /// A `usr/share/info/dir` file, or its `dir.old` backup, possibly gzipped.
    ///
    /// The name must match `usr/share/info/dir(.old)?(.gz)?` exactly.
    /// install-info generates these at install time, so a package must not
    /// ship them. Every installed entry is visited, in lintian's order.
    /// Extra is empty. The Location is the entry. Applies to deb, udeb, and
    /// ddeb.
    PackageContainsInfoDirFile,
    code = "LN0895",
    tag = "package-contains-info-dir-file",
    default_selected = true,
    experimental = false,
}

static INFO_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/info/dir(?:\.old)?(?:\.gz)?$").expect("valid regex"));

impl BinaryPackageRule for PackageContainsInfoDirFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits: Vec<_> = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            visit(files)
                .into_iter()
                .filter(|(_, name)| INFO_DIR.is_match(name))
                .map(|(entry, _)| entry.path().to_path_buf())
                .collect()
        };
        for path in hits {
            emit(ctx, path, Severity::Error);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn one(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("x"))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            one("usr/share/info/foo.info.gz"),
            one("usr/share/info/dir.txt"),
            one("usr/share/info/directory"),
            one("usr/share/info/old/dir"),
            one("usr/share/doc/dir"),
            one("usr/share/info/dir.bak"),
            BinaryPackageCase::from(vec![("usr/share/info/dir", TestFile::directory())]),
        ];
        let fail = vec![
            one("usr/share/info/dir"),
            one("usr/share/info/dir.gz"),
            one("usr/share/info/dir.old"),
            one("usr/share/info/dir.old.gz"),
            BinaryPackageCase::from(vec![("usr/share/info/dir", TestFile::symlink("other"))]),
            one("usr/share/info/dir.gz").udeb(),
        ];
        BinaryPackageTester::new(PackageContainsInfoDirFile, pass, fail).test_and_snapshot();
    }
}
