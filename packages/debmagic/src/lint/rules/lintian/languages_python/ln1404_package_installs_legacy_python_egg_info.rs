use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct PackageInstallsLegacyPythonEggInfo;

declare_rule! {
    /// A path contains `.egg-info` and sits under `usr/lib/python3/dist-packages`,
    /// `usr/lib/pyshared`, or `usr/share/`. That is what `setup.py install`
    /// leaves behind; `.dist-info` is the modern replacement.
    ///
    /// Every Data files entry is checked, directories included. Reported at the
    /// path with an empty extra. Udebs are checked too.
    PackageInstallsLegacyPythonEggInfo,
    code = "LN1404",
    tag = "package-installs-legacy-python-egg-info",
    default_selected = false,
    experimental = false,
}

const PREFIXES: &[&str] = &[
    "usr/lib/python3/dist-packages",
    "usr/lib/pyshared",
    "usr/share/",
];

impl BinaryPackageRule for PackageInstallsLegacyPythonEggInfo {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter_map(|entry| {
                // lintian names a directory with a trailing slash.
                let mut name = entry.path().as_os_str().as_bytes().to_vec();
                if entry.kind() == EntryKind::Directory {
                    name.push(b'/');
                }
                let prefixed = PREFIXES
                    .iter()
                    .any(|prefix| name.starts_with(prefix.as_bytes()));
                (prefixed && contains(&name, b".egg-info"))
                    .then(|| PathBuf::from(OsStr::from_bytes(&name)))
            })
            .collect();
        for path in paths {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            tree(vec![(
                "usr/lib/python3/dist-packages/foo.dist-info",
                TestFile::directory(),
            )]),
            tree(vec![(
                "usr/lib/python3/dist-packages/foo.dist-info/METADATA",
                TestFile::new("x"),
            )]),
            // Not under one of the prefixes.
            tree(vec![(
                "usr/lib/python3.12/foo.egg-info",
                TestFile::new("x"),
            )]),
            tree(vec![("opt/usr/share/foo.egg-info", TestFile::new("x"))]),
        ];
        let fail = vec![
            tree(vec![(
                "usr/lib/python3/dist-packages/foo.egg-info",
                TestFile::new("x"),
            )]),
            tree(vec![(
                "usr/lib/pyshared/foo.egg-info",
                TestFile::directory(),
            )]),
            tree(vec![(
                "usr/share/foo/bar.egg-info/PKG-INFO",
                TestFile::new("x"),
            )]),
        ];
        BinaryPackageTester::new(PackageInstallsLegacyPythonEggInfo, pass, fail)
            .test_and_snapshot();
    }
}
