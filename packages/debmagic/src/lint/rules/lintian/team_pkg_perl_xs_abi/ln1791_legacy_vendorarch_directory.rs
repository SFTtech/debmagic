use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct LegacyVendorarchDirectory;

declare_rule! {
    /// A Perl package built for perl 5.19.11 or later installs a file under
    /// `usr/lib/perl5/`, the pre-5.20 vendor directory.
    ///
    /// Lintian's `relies_on_modern_api`: the package is not `Architecture:
    /// all`, and its strong relation (`Pre-Depends` and `Depends`, every
    /// alternative) names `perlapi-<version>` with a version at or above
    /// `5.19.11`. Only the first such `perlapi-` prerequisite counts. When it
    /// holds, every regular file and hardlink under `usr/lib/perl5/` is
    /// reported with an empty extra, as lintian's hint has none. The path is
    /// the Location. Directories and symlinks are not files. Udebs are checked
    /// too. The version comparison is dpkg's for digits and dots, as
    /// `perlapi-` versions are.
    LegacyVendorarchDirectory,
    code = "LN1791",
    tag = "legacy-vendorarch-directory",
    default_selected = true,
    experimental = false,
}

/// The perl API from which the XS vendor directory is `$Config{vendorarch}`,
/// not `usr/lib/perl5`.
const MODERN_PERL_API: &str = "5.19.11";

static PERL_API: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^perlapi-(\d[\d.]*)$").expect("valid"));

/// Lintian's `relies_on_modern_api`.
fn relies_on_modern_api(ctx: &BinaryPackageContext<'_>) -> bool {
    let fields = ctx.installation_control().fields();
    if fields.get("Architecture").unwrap_or("").trim() == "all" {
        return false;
    }
    let strong = fields.relations_and(&["Pre-Depends", "Depends"]);
    let api = strong.alternatives().find_map(|alternative| {
        PERL_API
            .captures(alternative.package())
            .and_then(|captures| captures.get(1))
            .map(|api| api.as_str().to_string())
    });
    api.is_some_and(|api| version_at_least(&api, MODERN_PERL_API))
}

/// dpkg's `>=` for dotted numbers: missing components count as `0`.
fn version_at_least(version: &str, minimum: &str) -> bool {
    let parts = |text: &str| -> Vec<u64> {
        text.split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    };
    let (mut left, mut right) = (parts(version), parts(minimum));
    let len = left.len().max(right.len());
    left.resize(len, 0);
    right.resize(len, 0);
    left >= right
}

impl BinaryPackageRule for LegacyVendorarchDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !relies_on_modern_api(ctx) {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                entry.is_file()
                    && entry
                        .path()
                        .as_os_str()
                        .as_encoded_bytes()
                        .starts_with(b"usr/lib/perl5/")
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
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
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(depends: &str, files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
            .field("Architecture", "amd64")
            .field("Depends", depends)
    }

    fn module() -> TestFile {
        TestFile::new("package Foo;\n1;\n")
    }

    #[test]
    fn binary() {
        let pass = vec![
            package("libc6", vec![]),
            // No `perlapi-` prerequisite: lintian does not rely on it.
            package("libc6", vec![("usr/lib/perl5/Foo.pm", module())]),
            // Older perl API: the pre-5.19.11 directory is right for it.
            package("perlapi-5.18.2", vec![("usr/lib/perl5/Foo.pm", module())]),
            package("perlapi-5.19.10", vec![("usr/lib/perl5/Foo.pm", module())]),
            // `Architecture: all` never relies on the API.
            BinaryPackageCase::from(vec![("usr/lib/perl5/Foo.pm", module())])
                .field("Architecture", "all")
                .field("Depends", "perlapi-5.36.0"),
            // Only `Pre-Depends` and `Depends` make the strong relation.
            BinaryPackageCase::from(vec![("usr/lib/perl5/Foo.pm", module())])
                .field("Architecture", "amd64")
                .field("Recommends", "perlapi-5.36.0"),
            // Not under `usr/lib/perl5/`.
            package(
                "perlapi-5.36.0",
                vec![(
                    "usr/lib/x86_64-linux-gnu/perl5/5.36/auto/Foo/Foo.so",
                    module(),
                )],
            ),
            // `usr/lib/perl5x/` and the directory itself are not `usr/lib/perl5/`.
            package("perlapi-5.36.0", vec![("usr/lib/perl5x/Foo.pm", module())]),
            package(
                "perlapi-5.36.0",
                vec![("usr/lib/perl5", TestFile::directory())],
            ),
            // Directories and symlinks are not files.
            package(
                "perlapi-5.36.0",
                vec![("usr/lib/perl5/Foo", TestFile::directory())],
            ),
            package(
                "perlapi-5.36.0",
                vec![(
                    "usr/lib/perl5/Foo.pm",
                    TestFile::symlink("/usr/share/perl5/Foo.pm"),
                )],
            ),
            // A non-version prerequisite is not `perlapi-`.
            package("perlapi-dev", vec![("usr/lib/perl5/Foo.pm", module())]),
        ];
        let fail = vec![
            package("perlapi-5.36.0", vec![("usr/lib/perl5/Foo.pm", module())]),
            // Equal to the threshold.
            package("perlapi-5.19.11", vec![("usr/lib/perl5/Foo.pm", module())]),
            // `Pre-Depends` is part of the strong relation.
            BinaryPackageCase::from(vec![("usr/lib/perl5/Foo.pm", module())])
                .field("Architecture", "amd64")
                .field("Pre-Depends", "perlapi-5.20.1")
                .field("Depends", "libc6"),
            // Every alternative of an OR clause is visited.
            package(
                "libfoo | perlapi-5.36.0",
                vec![("usr/lib/perl5/Foo.pm", module())],
            ),
            // Every file under the directory is reported.
            package(
                "perlapi-5.36.0",
                vec![
                    ("usr/lib/perl5/Foo.pm", module()),
                    ("usr/lib/perl5/auto/Foo/Foo.so", module()),
                ],
            ),
            // Hardlinks are files to lintian.
            package(
                "perlapi-5.36.0",
                vec![("usr/lib/perl5/Foo.pm", module().hardlink())],
            ),
            // Udebs are checked too.
            package("perlapi-5.36.0", vec![("usr/lib/perl5/Foo.pm", module())]).udeb(),
            // Non-UTF-8 file name.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/lib/perl5/Foo\xff.pm".to_vec()),
                module(),
            )])
            .field("Architecture", "amd64")
            .field("Depends", "perlapi-5.36.0"),
        ];
        BinaryPackageTester::new(LegacyVendorarchDirectory, pass, fail).test_and_snapshot();
    }
}
