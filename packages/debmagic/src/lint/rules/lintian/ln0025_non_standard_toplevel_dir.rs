use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NonStandardToplevelDir;

declare_rule! {
    /// The Binary package installs a top-level directory the FHS does not name.
    ///
    /// Flags a directory directly under the root other than `bin`, `boot`,
    /// `dev`, `etc`, `home`, `lib`, `mnt`, `opt`, `root`, `run`, `sbin`, `srv`,
    /// `sys`, `tmp`, `usr`, and `var`. Lintian judges `hurd`, `servers`, and
    /// anything named `lib*` (the aliased-location and multi-arch lib Tags)
    /// elsewhere, and the packages `base-files`, `hurd`, `hurd-udeb`, and
    /// `rootskel*` are exempt. Extra is empty; the directory, with a trailing slash, is the Location.
    NonStandardToplevelDir,
    code = "LN0025",
    tag = "non-standard-toplevel-dir",
    default_selected = true,
    experimental = false,
}

static STANDARD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:bin|boot|dev|etc|home|lib|mnt|opt|root|run|sbin|srv|sys|tmp|usr|var|hurd|servers|lib[a-z0-9]*)$")
        .expect("valid regex")
});
static EXEMPT_PACKAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:base-files$|hurd$|hurd-udeb$|rootskel(?:-bootfloppy)?)").expect("valid regex")
});

impl BinaryPackageRule for NonStandardToplevelDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if EXEMPT_PACKAGE.is_match(ctx.package_name()) {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() == EntryKind::Directory)
            .filter(|entry| {
                entry.path().to_str().is_some_and(|name| {
                    !name.is_empty() && !name.contains('/') && !STANDARD.is_match(name)
                })
            })
            .map(|entry| PathBuf::from(format!("{}/", entry.path().display())))
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

    /// Explicit directory entries, as a `.deb`'s data tarball has.
    fn dirs(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::directory()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            dirs(&[]),
            dirs(&["usr", "usr/bin", "etc", "var", "var/lib", "opt"]),
            dirs(&["bin", "sbin", "lib", "lib64", "libfoo", "libx32"]),
            dirs(&[
                "hurd", "servers", "sys", "boot", "dev", "run", "tmp", "mnt", "srv", "home", "root",
            ]),
            // A file at the top level is not a directory; nested directories are other Tags.
            BinaryPackageCase::from(vec![("README", TestFile::new("content"))]),
            dirs(&["usr/emul", "etc/emul"]),
            dirs(&["emul"]).package("base-files"),
            dirs(&["emul"]).package("hurd"),
            dirs(&["emul"]).package("rootskel-bootfloppy"),
        ];
        let fail = vec![
            dirs(&["new-top-level-dir"]),
            dirs(&["emul", "build"]),
            dirs(&["emul"]).package("hurd-extra"),
            dirs(&["emul"]).udeb(),
            dirs(&["Usr"]),
        ];
        BinaryPackageTester::new(NonStandardToplevelDir, pass, fail).test_and_snapshot();
    }
}
