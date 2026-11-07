use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsNonbinaryPerlInUsrLibPerl5;

declare_rule! {
    /// A `.pm` or `.pl` file is installed under `usr/lib/perl5` with no binary
    /// Perl module beside it.
    ///
    /// Paths are `usr/lib/perl5/…` and `usr/lib/<triplet>/perl5/…`. A `.bs` or
    /// `.so` anywhere in those directories suppresses the tag. Extra is empty.
    /// The Location is the source file.
    PackageInstallsNonbinaryPerlInUsrLibPerl5,
    code = "LN0385",
    tag = "package-installs-nonbinary-perl-in-usr-lib-perl5",
    default_selected = true,
    experimental = false,
}

static SOURCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/(?:[^/]+/)?perl5/.*\.p[lm]$").expect("valid regex"));
static BINARY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/(?:[^/]+/)?perl5/.*\.(?:bs|so)$").expect("valid regex"));

impl BinaryPackageRule for PackageInstallsNonbinaryPerlInUsrLibPerl5 {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut sources = Vec::new();
        let mut binaries = false;
        for entry in files.entries() {
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            if SOURCE.is_match(path) {
                sources.push(PathBuf::from(path));
            } else if BINARY.is_match(path) {
                binaries = true;
            }
        }
        if binaries {
            return;
        }
        for path in sources {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("x")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            files(&["usr/share/perl5/Foo.pm"]),
            files(&["usr/lib/perl/Foo.pm"]),
            files(&["usr/lib/perl5/Foo/Bar.pm", "usr/lib/perl5/auto/Foo/Bar.so"]),
        ];
        let fail = vec![
            files(&["usr/lib/perl5/Foo/Bar.pm"]),
            files(&["usr/lib/x86_64-linux-gnu/perl5/5.42/foo/bar.pm"]),
            files(&["usr/lib/perl5/Foo/ancient.pl"]),
        ];
        BinaryPackageTester::new(PackageInstallsNonbinaryPerlInUsrLibPerl5, pass, fail)
            .test_and_snapshot();
    }
}
