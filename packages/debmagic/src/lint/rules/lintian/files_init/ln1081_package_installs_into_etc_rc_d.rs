use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsIntoEtcRcD;

declare_rule! {
    /// The Binary package installs anything into `etc/rc.d/` or `etc/rc?.d/`
    /// (`rc0.d` to `rc9.d`, `rcS.d`).
    ///
    /// Walks every Data files entry (directories, symlinks and special files
    /// included, as lintian's installed items are) whose path is one of those
    /// directories followed by a non-space character. Skipped: the Binary
    /// package is `sysvinit` or `file-rc` (they own these directories), and
    /// a udeb. Extra is empty; the Location is the entry.
    PackageInstallsIntoEtcRcD,
    code = "LN1081",
    tag = "package-installs-into-etc-rc.d",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageInstallsIntoEtcRcD {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if matches!(ctx.package_name(), "sysvinit" | "file-rc") || ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| is_rc_d_entry(entry.path().as_os_str().as_bytes()))
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

/// Lintian's `^etc/rc(?:\d|S)?\.d/\S` on raw bytes.
fn is_rc_d_entry(name: &[u8]) -> bool {
    let Some(rest) = name.strip_prefix(b"etc/rc") else {
        return false;
    };
    let rest = match rest.first() {
        Some(byte) if byte.is_ascii_digit() || *byte == b'S' => &rest[1..],
        _ => rest,
    };
    rest.strip_prefix(b".d/")
        .and_then(|after| after.first())
        .is_some_and(|byte| !is_perl_space(*byte))
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
            BinaryPackageCase::from(vec![("etc/rc.local", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/rc.d/", TestFile::directory())]),
            BinaryPackageCase::from(vec![("etc/rc1.d-old/foo", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/rc.d/foo", TestFile::new("x"))]).package("sysvinit"),
            BinaryPackageCase::from(vec![("etc/rc2.d/S01foo", TestFile::new("x"))])
                .package("file-rc"),
            BinaryPackageCase::from(vec![("etc/rc.d/foo", TestFile::new("x"))]).udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("etc/rc.d/foo", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/rc.d/foo", TestFile::directory())]),
            BinaryPackageCase::from(vec![("etc/rc2.d/S01foo", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/rcS.d/S01foo", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![(
                "etc/rc3.d/K01foo",
                TestFile::symlink("../init.d/foo"),
            )]),
            BinaryPackageCase::from(vec![("etc/rc.d/foo", TestFile::new("x").hardlink())]),
            BinaryPackageCase::from(vec![("etc/rc.d/foo", TestFile::new("x"))]).package("foo"),
        ];
        BinaryPackageTester::new(PackageInstallsIntoEtcRcD, pass, fail).test_and_snapshot();
    }
}
