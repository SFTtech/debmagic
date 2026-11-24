use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsIntoEtcRcBoot;

declare_rule! {
    /// The Binary package installs anything below `etc/rc.boot/`, the obsolete
    /// boot directory.
    ///
    /// Walks every Data files entry (directories, symlinks and special files
    /// included, as lintian's installed items are) whose path starts with
    /// `etc/rc.boot/` followed by a non-space character. Extra is empty; the
    /// Location is the entry.
    PackageInstallsIntoEtcRcBoot,
    code = "LN1080",
    tag = "package-installs-into-etc-rc.boot",
    default_selected = true,
    experimental = false,
}

const PREFIX: &[u8] = b"etc/rc.boot/";

impl BinaryPackageRule for PackageInstallsIntoEtcRcBoot {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                entry
                    .path()
                    .as_os_str()
                    .as_bytes()
                    .strip_prefix(PREFIX)
                    .and_then(|rest| rest.first())
                    .is_some_and(|byte| !is_perl_space(*byte))
            })
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

/// Perl's `\s`: the ASCII white space bytes.
fn is_perl_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("etc/rc.d/foo", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/rc.boot", TestFile::directory())]),
            BinaryPackageCase::from(vec![("usr/etc/rc.boot/foo", TestFile::new("x"))]),
            // The empty name after the prefix is not a boot script.
            BinaryPackageCase::from(vec![("etc/rc.boot/", TestFile::directory())]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("etc/rc.boot/foo", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/rc.boot/foo", TestFile::directory())]),
            BinaryPackageCase::from(vec![(
                "etc/rc.boot/foo",
                TestFile::symlink("/usr/share/foo"),
            )]),
            BinaryPackageCase::from(vec![("etc/rc.boot/foo", TestFile::new("x").hardlink())]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("etc/rc.boot/foo", TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(PackageInstallsIntoEtcRcBoot, pass, fail).test_and_snapshot();
    }
}
