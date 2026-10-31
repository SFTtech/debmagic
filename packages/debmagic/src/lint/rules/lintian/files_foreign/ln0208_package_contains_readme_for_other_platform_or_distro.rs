use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::{emit, visit};

pub struct PackageContainsReadmeForOtherPlatformOrDistro;

declare_rule! {
    /// `usr/share/doc/<package>/readme.<platform>`, optionally `.txt` and `.gz`.
    ///
    /// The package name is the Installation control `Package` field, matched
    /// case-insensitively (`quotemeta` inside the Check's `/i` regex). The
    /// platform is one of apple, aix, atari, be, beos, bsd, bsdi, cygwin,
    /// darwin, irix, gentoo, freebsd, mac, macos, macosx, netbsd, openbsd,
    /// osf, redhat, sco, sgi, solaris, suse, sun, vms, win32, win9x, windows.
    /// The file must sit directly in that doc directory. Any entry kind whose
    /// index name matches; a directory's trailing slash does not. Skipped for
    /// udebs, because the Check returns first on `usr/share/doc/`. Extra is
    /// empty.
    PackageContainsReadmeForOtherPlatformOrDistro,
    code = "LN0208",
    tag = "package-contains-readme-for-other-platform-or-distro",
    default_selected = true,
    experimental = false,
}

const PLATFORMS: &str = "apple|aix|atari|be|beos|bsd|bsdi|cygwin|darwin|irix|gentoo|freebsd|mac|macos|macosx|netbsd|openbsd|osf|redhat|sco|sgi|solaris|suse|sun|vms|win32|win9x|windows";

impl BinaryPackageRule for PackageContainsReadmeForOtherPlatformOrDistro {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let pattern = readme_pattern(ctx.package_name());
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<String> = visit(files)
            .into_iter()
            .filter_map(|(_, name)| pattern.is_match(&name).then_some(name))
            .collect();
        for path in hits {
            emit(ctx, path, Severity::Warning);
        }
    }
}

fn readme_pattern(package: &str) -> Regex {
    let package = regex::escape(package);
    Regex::new(&format!(
        "(?i)^usr/share/doc/{package}/readme[.](?:{PLATFORMS})(?:[.]txt)?(?:[.]gz)?$"
    ))
    .expect("valid regex")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(paths: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let readme = TestFile::new("x");
        let pass = vec![
            files(&[
                ("usr/share/doc/lib++/README.debian", readme.clone()),
                ("usr/share/doc/lib++/README.linux", readme.clone()),
                ("usr/share/doc/lib++/doc/README.windows", readme.clone()),
                ("usr/share/doc/other/README.windows", readme.clone()),
                ("usr/share/doc/lib+++/README.windows", readme.clone()),
                ("usr/share/doc/lib++/README.windows", TestFile::directory()),
            ])
            .package("lib++"),
            files(&[("usr/share/doc/example/README.windows", readme.clone())]).udeb(),
        ];
        let fail = vec![
            files(&[
                ("usr/share/doc/filenames/README.macosx", readme.clone()),
                ("usr/share/doc/filenames/README.windows", readme.clone()),
                ("usr/share/doc/filenames/README.mac", readme.clone()),
                (
                    "usr/share/doc/filenames/readme.WINDOWS.txt.gz",
                    readme.clone(),
                ),
                ("usr/share/doc/filenames/README.beos", readme.clone()),
                ("usr/share/doc/filenames/README.be", readme),
            ])
            .package("filenames"),
            files(&[(
                "usr/share/doc/lib++/README.sun",
                TestFile::symlink("elsewhere"),
            )])
            .package("Lib++"),
        ];
        BinaryPackageTester::new(PackageContainsReadmeForOtherPlatformOrDistro, pass, fail)
            .test_and_snapshot();
    }
}
