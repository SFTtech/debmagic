use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsPythonEgg;

/// Where lintian accepts a `.egg`: a versioned `usr/lib/pythonX.Y/` tree,
/// `usr/lib/pyshared`, or anything under `usr/share/`.
static EGG_LOCATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/python[0-9]+(?:\.[0-9]+/)|^usr/lib/pyshared|^usr/share/")
        .expect("valid regex")
});

/// lintian's `$item->name`: the path, with a trailing `/` for a directory.
fn lintian_name(entry: &Entry) -> OsString {
    let mut name = entry.path().as_os_str().to_os_string();
    if entry.is_dir() {
        name.push("/");
    }
    name
}

declare_rule! {
    /// The package installs a Python egg (a path ending in `.egg`).
    ///
    /// Reported when the path is under `usr/lib/pythonX.Y/`, under
    /// `usr/lib/pyshared`, or under `usr/share/`. Each such entry is reported
    /// with its path as the Location; extra is empty. A directory whose name
    /// ends in `.egg` is not reported, because lintian's name for it ends in
    /// `/`. Symlinks and hardlinks are reported. Udebs are checked too.
    PackageInstallsPythonEgg,
    code = "LN1114",
    tag = "package-installs-python-egg",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageInstallsPythonEgg {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter_map(|entry| {
                let name = lintian_name(entry);
                let bytes = name.as_encoded_bytes();
                let egg = bytes.ends_with(b".egg") && EGG_LOCATION.is_match(bytes);
                egg.then(|| PathBuf::from(name))
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
                // Not under one of the three locations.
                ("usr/lib/foo.egg".to_string(), TestFile::new("x")),
                // `usr/lib/python3/` has no `.Y/` part, so it does not match.
                (
                    "usr/lib/python3/dist-packages/foo.egg".to_string(),
                    TestFile::new("x"),
                ),
            ]),
            // A directory named `*.egg` is not reported (lintian's name ends in `/`).
            BinaryPackageCase::from(vec![(
                "usr/share/foo/bar.egg".to_string(),
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/python-kinterbasdb/python.egg".to_string(),
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/python3.12/site-packages/foo.egg".to_string(),
                    TestFile::new("x"),
                ),
                ("usr/lib/pyshared/foo.egg".to_string(), TestFile::new("x")),
            ]),
            // A symlink is not a directory, so it is reported.
            BinaryPackageCase::from(vec![(
                "usr/share/foo/link.egg".to_string(),
                TestFile::symlink("foo.egg"),
            )]),
            // Udeb packages are checked as well.
            BinaryPackageCase::from(vec![(
                "usr/share/foo/foo.egg".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageInstallsPythonEgg, pass, fail).test_and_snapshot();
    }
}
