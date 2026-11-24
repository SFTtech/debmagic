use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsPacklist;

declare_rule! {
    /// A `.packlist` file is installed under `usr/lib/perl…`. MakeMaker leaves
    /// these behind; they are useless, and can put an architecture-specific
    /// directory name in an architecture-independent package.
    ///
    /// Matches lintian's `^usr/lib/perl.*/.packlist$` on the raw path bytes,
    /// including its unescaped `.`, so any single character before `packlist`
    /// counts. Directories are skipped, as lintian names them with a trailing
    /// `/`. Extra is empty. The Location is the file.
    PackageInstallsPacklist,
    code = "LN1107",
    tag = "package-installs-packlist",
    default_selected = true,
    experimental = false,
}

static PACKLIST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)^usr/lib/perl.*/.packlist$").expect("valid regex"));

impl BinaryPackageRule for PackageInstallsPacklist {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| !entry.is_dir())
            .filter(|entry| PACKLIST.is_match(entry.path().as_os_str().as_bytes()))
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
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("\n"))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            file("usr/share/perl5/.packlist"),
            file("usr/lib/perl5/packlist"),
            // The path must start `usr/lib/perl`; a triplet directory does not count.
            file("usr/lib/x86_64-linux-gnu/perl/5.42/auto/Foo/.packlist"),
            BinaryPackageCase::from(vec![("usr/lib/perl5/.packlist", TestFile::directory())]),
        ];
        let fail = vec![
            file("usr/lib/perl5/.packlist"),
            file("usr/lib/perl-foo/.packlist"),
            // lintian's unescaped `.` matches any character before `packlist`.
            file("usr/lib/perl5/Xpacklist"),
            BinaryPackageCase::from(vec![("usr/lib/perl5/.packlist", TestFile::symlink("x"))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/perl5/.packlist",
                TestFile::new("\n").hardlink(),
            )]),
            file("usr/lib/perl5/.packlist").udeb(),
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/lib/perl-\xff/.packlist".to_vec()),
                TestFile::new("\n"),
            )]),
        ];
        BinaryPackageTester::new(PackageInstallsPacklist, pass, fail).test_and_snapshot();
    }
}
