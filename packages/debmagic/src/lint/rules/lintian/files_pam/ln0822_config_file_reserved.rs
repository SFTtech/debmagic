use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct ConfigFileReserved;

declare_rule! {
    /// `etc/pam.conf` is installed by a package other than `libpam-runtime`.
    ///
    /// Walks the Data files entries at `etc/pam.conf`; a directory of that
    /// name is not a match. The package that owns the path is the only
    /// exception. Extra is `by libpam-runtime`; the path is the Location.
    /// Udebs are checked too.
    ConfigFileReserved,
    code = "LN0822",
    tag = "config-file-reserved",
    default_selected = true,
    experimental = false,
}

const RESERVED_PATH: &str = "etc/pam.conf";
const OWNER: &str = "libpam-runtime";

impl BinaryPackageRule for ConfigFileReserved {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name() == OWNER {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| !matches!(entry.kind(), EntryKind::Directory))
            .filter(|entry| entry.path() == Path::new(RESERVED_PATH))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(
                Diagnostic::error("by libpam-runtime").with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(package: &str, path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)]).package(package)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("libpam-runtime", "etc/pam.conf", TestFile::new("x")),
            data("example", "etc/pam.d/login", TestFile::new("x")),
            data("example", "etc/pam.confx", TestFile::new("x")),
            data("example", "etc/pam.conf/inner", TestFile::new("x")),
            data("example", "usr/share/etc/pam.conf", TestFile::new("x")),
            // A directory of that name is not the file.
            data("example", "etc/pam.conf", TestFile::directory()),
        ];
        let fail = vec![
            data("example", "etc/pam.conf", TestFile::new("x")),
            data("libpam-pkg", "etc/pam.conf", TestFile::new("x")),
            data("example", "etc/pam.conf", TestFile::symlink("pam.d/other")),
            data("example", "etc/pam.conf", TestFile::new("x")).udeb(),
        ];
        BinaryPackageTester::new(ConfigFileReserved, pass, fail).test_and_snapshot();
    }
}
