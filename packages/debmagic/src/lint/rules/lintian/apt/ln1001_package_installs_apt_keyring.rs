use std::os::unix::ffi::OsStrExt;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsAptKeyring;

declare_rule! {
    /// The Binary package installs a file under `etc/apt/trusted.gpg.d/` or
    /// installs `etc/apt/trusted.gpg`, the local administrator's keyrings.
    ///
    /// Walks every Data file entry (directories and symlinks included, as
    /// lintian's installed items are) whose raw path starts with
    /// `etc/apt/trusted.gpg`, followed by the end of the path or by `.d/` and
    /// at least one more byte. Packages whose name ends with `-apt-source` or
    /// `-archive-keyring` are exempt, and so is the Source package `apt`. Extra
    /// is empty; the path is the Location.
    PackageInstallsAptKeyring,
    code = "LN1001",
    tag = "package-installs-apt-keyring",
    default_selected = true,
    experimental = false,
}

static KEYRING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/apt/trusted\.gpg(?:$|\.d/[^/]+)").expect("valid regex"));

impl BinaryPackageRule for PackageInstallsAptKeyring {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.source_name() == "apt" {
            return;
        }
        let name = ctx.package_name();
        if name.ends_with("-apt-source") || name.ends_with("-archive-keyring") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| KEYRING.is_match(entry.path().as_os_str().as_bytes()))
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
            BinaryPackageCase::from(vec![("etc/apt/trusted.gpg.d/bar.gpg", TestFile::new("x"))])
                .package("foo-apt-source"),
            BinaryPackageCase::from(vec![("etc/apt/trusted.gpg.d/bar.gpg", TestFile::new("x"))])
                .package("foo-archive-keyring"),
            BinaryPackageCase::from(vec![("etc/apt/trusted.gpg.d/bar.gpg", TestFile::new("x"))])
                .field("Source", "apt"),
            BinaryPackageCase::from(vec![("etc/apt/trusted.gpg.dx/bar.gpg", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/apt/trusted.gpg.d", TestFile::directory())]),
            BinaryPackageCase::from(vec![("usr/share/keyrings/bar.gpg", TestFile::new("x"))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("etc/apt/trusted.gpg", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/apt/trusted.gpg.d/bar.gpg", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![(
                "etc/apt/trusted.gpg.d/sub/bar.gpg",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "etc/apt/trusted.gpg.d/bar.gpg",
                TestFile::new("x").hardlink(),
            )]),
            BinaryPackageCase::from(vec![(
                "etc/apt/trusted.gpg",
                TestFile::symlink("/usr/share/keyrings/bar.gpg"),
            )]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("etc/apt/trusted.gpg", TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(PackageInstallsAptKeyring, pass, fail).test_and_snapshot();
    }
}
