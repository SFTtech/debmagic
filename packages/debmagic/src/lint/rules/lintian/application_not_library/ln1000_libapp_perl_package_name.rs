use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct LibappPerlPackageName;

declare_rule! {
    /// The Binary package is named `libapp-*-perl` but ships a program in `$PATH`.
    ///
    /// "This package contains a program in $PATH and is named libapp-*-perl which
    /// usually implies that the upstream project on CPAN is under the App::
    /// hierarchy for applications. Instead of libfoo-bar-perl it should be named
    /// foo-bar."
    ///
    /// Each direct child of `bin`, `sbin`, `usr/bin`, `usr/sbin` and `usr/games` is
    /// a program, except debhelper plugins (`dh_*`) and names ending in `properties`
    /// or `update`. Lintian's exception list of Binary packages (perl, ruby, python,
    /// cpan tools, `libmodule-*-perl`, `lib*-dev` and similar, and the whitelist) is
    /// skipped entirely. Names are matched as bytes, so non-UTF-8 program names are
    /// reported. The extra is empty; each program is a Location.
    LibappPerlPackageName,
    code = "LN1000",
    tag = "libapp-perl-package-name",
    default_selected = true,
    experimental = false,
}

/// The directories lintian looks for programs in.
const PROGRAM_FOLDERS: [&str; 5] = ["bin", "sbin", "usr/bin", "usr/sbin", "usr/games"];

/// lintian's exception list, as anchored in its `installable`.
static EXEMPT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:perl(?:-base)?|ruby[\d.]*|python[\d.]*(?:-dev|-minimal)?|python[\d.]*-(?:stdeb|setuptools)|libmodule-.*-perl|libdevel-.*-perl|libperl.*-perl|libtest-.*-perl)$|^(?:cpan|gem2deb|xulrunner)|^lib.*-(?:utils|tools|bin|dev)",
    )
    .expect("valid regex")
});

/// lintian's whitelist of exact names.
const WHITELIST: [&str; 5] = [
    "rake",
    "bundler",
    "coderay",
    "kdelibs-bin",
    "libapp-options-perl",
];

static LIBAPP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^libapp.+-perl$").expect("valid regex"));

impl BinaryPackageRule for LibappPerlPackageName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        if !LIBAPP.is_match(&package)
            || EXEMPT.is_match(&package)
            || WHITELIST.contains(&package.as_str())
        {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let programs: Vec<PathBuf> = PROGRAM_FOLDERS
            .iter()
            .flat_map(|folder| {
                files
                    .entries()
                    .iter()
                    .filter(move |entry| entry.path().parent() == Some(Path::new(folder)))
                    .filter(|entry| is_program(entry.path()))
                    .map(|entry| entry.path().to_path_buf())
            })
            .collect();
        for path in programs {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Not a debhelper plugin, a `*properties` or an `*update` helper.
fn is_program(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().as_bytes();
    let full = path.as_os_str().as_bytes();
    !(name.starts_with(b"dh_") || full.ends_with(b"properties") || full.ends_with(b"update"))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(package: &str, paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| ((*path).to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
        .package(package)
    }

    #[test]
    fn binary() {
        let pass = vec![
            files("libapp-something-perl", &[]),
            files(
                "libapp-something-perl",
                &["usr/share/doc/libapp-something-perl/README"],
            ),
            // Not a program folder.
            files(
                "libapp-something-perl",
                &["usr/lib/libapp-something-perl/bin.pl"],
            ),
            // Nested below a program folder is not a direct child.
            files(
                "libapp-something-perl",
                &["usr/bin/sub/libapp-something-perl"],
            ),
            // Excluded names.
            files(
                "libapp-something-perl",
                &[
                    "usr/bin/dh_something",
                    "usr/bin/libapp-something-properties",
                    "usr/bin/libapp-something-update",
                ],
            ),
            // Not named libapp-*-perl.
            files(
                "libsomething-else-perl",
                &["usr/bin/libsomething-else-perl"],
            ),
            files("libapp-perl", &["usr/bin/libapp-perl"]),
            files(
                "libapp-something-perl-data",
                &["usr/bin/libapp-something-perl-data"],
            ),
            // Exception list and whitelist.
            files("libapp-options-perl", &["usr/bin/libapp-options-perl"]),
            files("libapp-dev-perl", &["usr/bin/libapp-dev-perl"]),
            files("libapp-x-tools-perl", &["usr/bin/libapp-x-tools-perl"]),
            files("libsomething-perl", &["usr/bin/libsomething-perl"]),
        ];
        let fail = vec![
            files("libapp-something-perl", &["usr/bin/libapp-something-perl"]),
            files("libapp-something-perl", &["bin/libapp-something-perl"]),
            files("libapp-something-perl", &["usr/sbin/tool"]),
            files("libapp-something-perl", &["usr/games/tool"]),
            files(
                "libapp-something-perl",
                &["usr/bin/tool-a", "usr/bin/tool-b", "usr/sbin/tool-c"],
            ),
            // Non-UTF-8 program name.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/bin/tool-\xff".to_vec()),
                TestFile::from_bytes("content"),
            )])
            .package("libapp-something-perl"),
        ];
        BinaryPackageTester::new(LibappPerlPackageName, pass, fail).test_and_snapshot();
    }
}
