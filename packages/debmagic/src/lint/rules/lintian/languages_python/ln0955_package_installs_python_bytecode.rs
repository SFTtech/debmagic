use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsPythonBytecode;

/// lintian skips names containing `/__pycache__/` (the slash on both sides).
const PYCACHE_SEGMENT: &[u8] = b"/__pycache__/";

declare_rule! {
    /// The package installs compiled Python files (`*.pyc` or `*.pyo`).
    ///
    /// Each such file is reported with its path as the Location; extra is
    /// empty. Files inside a `__pycache__` directory are skipped here, since
    /// `package-installs-python-pycache-dir` covers them. Directories are not
    /// reported. Udebs are checked too.
    PackageInstallsPythonBytecode,
    code = "LN0955",
    tag = "package-installs-python-bytecode",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageInstallsPythonBytecode {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter(|entry| {
                let bytes = entry.path().as_os_str().as_encoded_bytes();
                let compiled = bytes.ends_with(b".pyc") || bytes.ends_with(b".pyo");
                let in_pycache = bytes
                    .windows(PYCACHE_SEGMENT.len())
                    .any(|window| window == PYCACHE_SEGMENT);
                compiled && !in_pycache
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

    #[test]
    fn binary() {
        let pass = vec![BinaryPackageCase::from(vec![
            ("usr/share/foo/code.py".to_string(), TestFile::new("x")),
            (
                "usr/share/foo/__pycache__/code.cpython-313.pyc".to_string(),
                TestFile::new("x"),
            ),
            (
                "usr/share/foo/pyc-notes.txt".to_string(),
                TestFile::new("x"),
            ),
        ])];
        let fail = vec![
            BinaryPackageCase::from(vec![
                ("usr/share/foo/code.pyc".to_string(), TestFile::new("x")),
                ("usr/share/foo/code.pyo".to_string(), TestFile::new("x")),
            ]),
            // Udeb packages are checked as well.
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/code.pyc".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
            // The `/__pycache__/` skip needs a parent segment, so a top-level one is reported.
            BinaryPackageCase::from(vec![(
                "__pycache__/code.pyc".to_string(),
                TestFile::new("x"),
            )]),
        ];
        BinaryPackageTester::new(PackageInstallsPythonBytecode, pass, fail).test_and_snapshot();
    }
}
