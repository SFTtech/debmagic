use std::os::unix::ffi::OsStrExt;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsAptSources;

declare_rule! {
    /// The Binary package installs a file under `etc/apt/sources.list` or
    /// `etc/apt/sources.list.d/`, the local administrator's apt sources.
    ///
    /// Walks every Data file entry (directories and symlinks included, as
    /// lintian's installed items are) whose raw path starts with
    /// `etc/apt/sources.list`, followed by the end of the path or by `.d/` and
    /// at least one more byte. Packages whose name ends with `-apt-source` are
    /// exempt, and so is the Source package `apt`. Extra is empty; the path is
    /// the Location.
    PackageInstallsAptSources,
    code = "LN1003",
    tag = "package-installs-apt-sources",
    default_selected = true,
    experimental = false,
}

static SOURCES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/apt/sources\.list(?:$|\.d/[^/]+)").expect("valid regex"));

impl BinaryPackageRule for PackageInstallsAptSources {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.source_name() == "apt" || ctx.package_name().ends_with("-apt-source") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| SOURCES.is_match(entry.path().as_os_str().as_bytes()))
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
            BinaryPackageCase::from(vec![(
                "etc/apt/sources.list.d/bar.list",
                TestFile::new("x"),
            )])
            .package("foo-apt-source"),
            BinaryPackageCase::from(vec![(
                "etc/apt/sources.list.d/bar.list",
                TestFile::new("x"),
            )])
            .field("Source", "apt"),
            BinaryPackageCase::from(vec![(
                "etc/apt/sources.list.dx/bar.list",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![("etc/apt/sources.list.d", TestFile::directory())]),
            BinaryPackageCase::from(vec![("usr/share/apt/sources.list", TestFile::new("x"))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("etc/apt/sources.list", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![(
                "etc/apt/sources.list.d/bar.list",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "etc/apt/sources.list.d/sub/bar.list",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "etc/apt/sources.list.d/bar.list",
                TestFile::new("x").hardlink(),
            )]),
            BinaryPackageCase::from(vec![(
                "etc/apt/sources.list",
                TestFile::symlink("/usr/share/apt/sources.list"),
            )]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("etc/apt/sources.list", TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(PackageInstallsAptSources, pass, fail).test_and_snapshot();
    }
}
