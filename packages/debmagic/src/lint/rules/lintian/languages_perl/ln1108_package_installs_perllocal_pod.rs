use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsPerllocalPod;

declare_rule! {
    /// A `perllocal.pod` file is installed under `usr/lib/perl…`. Installing it
    /// wipes out the local documentation that already exists there.
    ///
    /// Matches lintian's `^usr/lib/perl.*/perllocal.pod$` on the raw path
    /// bytes, including its unescaped `.`. Directories are skipped, as lintian
    /// names them with a trailing `/`. Extra is empty. The Location is the file.
    PackageInstallsPerllocalPod,
    code = "LN1108",
    tag = "package-installs-perllocal-pod",
    default_selected = true,
    experimental = false,
}

static PERLLOCAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)^usr/lib/perl.*/perllocal.pod$").expect("valid regex"));

impl BinaryPackageRule for PackageInstallsPerllocalPod {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| !entry.is_dir())
            .filter(|entry| PERLLOCAL.is_match(entry.path().as_os_str().as_bytes()))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
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
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("pod\n"))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            file("usr/lib/perllocal.pod"),
            file("usr/share/perl/perllocal.pod"),
            file("usr/lib/perl/perllocal.pod.gz"),
            // The path must start `usr/lib/perl`; a triplet directory does not count.
            file("usr/lib/x86_64-linux-gnu/perl/5.42/perllocal.pod"),
            BinaryPackageCase::from(vec![(
                "usr/lib/perl-foo/perllocal.pod",
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            file("usr/lib/perl/perllocal.pod"),
            file("usr/lib/perl-foo/perllocal.pod"),
            // lintian's unescaped `.` matches any character before `pod`.
            file("usr/lib/perl-foo/perllocalxpod"),
            BinaryPackageCase::from(vec![(
                "usr/lib/perl-foo/perllocal.pod",
                TestFile::new("pod\n").hardlink(),
            )]),
            file("usr/lib/perl-foo/perllocal.pod").udeb(),
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/lib/perl-\xff/perllocal.pod".to_vec()),
                TestFile::new("pod\n"),
            )]),
        ];
        BinaryPackageTester::new(PackageInstallsPerllocalPod, pass, fail).test_and_snapshot();
    }
}
