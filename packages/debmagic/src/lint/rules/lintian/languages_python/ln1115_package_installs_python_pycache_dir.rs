use std::ffi::OsString;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsPythonPycacheDir;

/// lintian matches `/__pycache__/` against its name, which ends in `/`.
const PYCACHE_SEGMENT: &[u8] = b"/__pycache__/";

/// lintian's `$item->name`: the path, with a trailing `/` for a directory.
fn lintian_name(entry: &Entry) -> OsString {
    let mut name = entry.path().as_os_str().to_os_string();
    if entry.is_dir() {
        name.push("/");
    }
    name
}

declare_rule! {
    /// The package installs a `__pycache__` directory.
    ///
    /// Each directory whose path contains a `/__pycache__/` segment is
    /// reported with its path, with a trailing slash, as the Location; extra
    /// is empty. A `__pycache__` directory at the top level of the package is
    /// not reported, since there is no `/` before it. Files are not reported
    /// here (see `package-installs-python-bytecode`). Udebs are checked too.
    PackageInstallsPythonPycacheDir,
    code = "LN1115",
    tag = "package-installs-python-pycache-dir",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageInstallsPythonPycacheDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_dir())
            .filter_map(|entry| {
                let name = lintian_name(entry);
                let bytes = name.as_encoded_bytes();
                let pycache = bytes
                    .windows(PYCACHE_SEGMENT.len())
                    .any(|window| window == PYCACHE_SEGMENT);
                pycache.then(|| PathBuf::from(name))
            })
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

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(vec![
                ("usr/share/foo/code.py".to_string(), TestFile::new("x")),
                // A file named `__pycache__` is not a directory.
                ("usr/share/foo/__pycache__".to_string(), TestFile::new("x")),
                // A top-level `__pycache__` has no `/` before it.
                ("__pycache__".to_string(), TestFile::directory()),
            ]),
            // A symlink named `__pycache__` is not a directory.
            BinaryPackageCase::from(vec![(
                "usr/share/foo/__pycache__".to_string(),
                TestFile::symlink("elsewhere"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/python-kinterbasdb/__pycache__".to_string(),
                TestFile::directory(),
            )]),
            // Nested directories and files inside are each judged on their own.
            BinaryPackageCase::from(vec![
                ("usr/lib/a/b/__pycache__".to_string(), TestFile::directory()),
                (
                    "usr/lib/a/b/__pycache__/code.cpython-313.pyc".to_string(),
                    TestFile::new("x"),
                ),
            ]),
            // Udeb packages are checked as well.
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/__pycache__".to_string(),
                TestFile::directory(),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageInstallsPythonPycacheDir, pass, fail).test_and_snapshot();
    }
}
