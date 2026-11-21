use std::os::unix::ffi::OsStrExt;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsAptPreferences;

declare_rule! {
    /// The Binary package installs a file under `etc/apt/preferences` or
    /// `etc/apt/preferences.d/`, the local administrator's apt pin priorities.
    ///
    /// Walks every Data file entry (directories and symlinks included, as
    /// lintian's installed items are) whose raw path starts with
    /// `etc/apt/preferences`, followed by the end of the path or by `.d/` and at
    /// least one more byte. The Source package `apt` is exempt. Extra is empty;
    /// the path is the Location.
    PackageInstallsAptPreferences,
    code = "LN1002",
    tag = "package-installs-apt-preferences",
    default_selected = true,
    experimental = false,
}

static PREFERENCES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/apt/preferences(?:$|\.d/[^/]+)").expect("valid regex"));

impl BinaryPackageRule for PackageInstallsAptPreferences {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.source_name() == "apt" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| PREFERENCES.is_match(entry.path().as_os_str().as_bytes()))
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
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("etc/apt/preferences.d/bar", TestFile::new("x"))])
                .field("Source", "apt"),
            BinaryPackageCase::from(vec![("etc/apt/preferences.dx/bar", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/apt/preferences.d", TestFile::directory())]),
            BinaryPackageCase::from(vec![("usr/share/apt/preferences", TestFile::new("x"))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("etc/apt/preferences", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/apt/preferences.d/bar", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/apt/preferences.d/sub/bar", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![(
                "etc/apt/preferences.d/bar",
                TestFile::new("x").hardlink(),
            )]),
            BinaryPackageCase::from(vec![(
                "etc/apt/preferences",
                TestFile::symlink("/usr/share/apt/preferences"),
            )]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("etc/apt/preferences", TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(PackageInstallsAptPreferences, pass, fail).test_and_snapshot();
    }
}
