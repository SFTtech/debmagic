use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct NonStandardFilePermissionsForEtcInitDScript;

declare_rule! {
    /// A regular file or hardlink under `etc/init.d/` has a mode other than
    /// `0755`.
    ///
    /// Walks Data files; `README` and `skeleton` directly in `etc/init.d/` are
    /// skipped, and so are directories, symlinks and special files. Extra is
    /// `<mode> != 0755` (the permission bits in four octal digits), the
    /// Location is the script. Setuid, setgid and sticky bits count, as they
    /// do in lintian's `operm`.
    NonStandardFilePermissionsForEtcInitDScript,
    code = "LN1078",
    tag = "non-standard-file-permissions-for-etc-init.d-script",
    default_selected = true,
    experimental = false,
}

const PREFIX: &[u8] = b"etc/init.d/";
const EXECUTABLE: u32 = 0o755;

impl BinaryPackageRule for NonStandardFilePermissionsForEtcInitDScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                let name = entry.path().as_os_str().as_bytes();
                let Some(rest) = name.strip_prefix(PREFIX) else {
                    return false;
                };
                rest.first().is_some_and(|byte| !is_perl_space(*byte))
                    && rest != b"README"
                    && rest != b"skeleton"
                    && entry.is_file()
                    && entry.mode() != EXECUTABLE
            })
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::error(format!("{mode:04o} != {EXECUTABLE:04o}")).with_location(
                    Location {
                        path,
                        line: None,
                        column: None,
                    },
                ),
            );
        }
    }
}

/// Perl's `\s`: the ASCII white space bytes.
fn is_perl_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("etc/init.d/foo", TestFile::new("x").mode(0o755))]),
            BinaryPackageCase::from(vec![("etc/init.d/README", TestFile::new("x").mode(0o644))]),
            BinaryPackageCase::from(vec![(
                "etc/init.d/skeleton",
                TestFile::new("x").mode(0o644),
            )]),
            BinaryPackageCase::from(vec![("etc/init.d/foo", TestFile::directory())]),
            BinaryPackageCase::from(vec![(
                "etc/init.d/foo",
                TestFile::symlink("/usr/share/foo"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/foo/etc/init.d/foo",
                TestFile::new("x").mode(0o644),
            )]),
            // The empty name after the prefix is not a script.
            BinaryPackageCase::from(vec![("etc/init.d/", TestFile::new("x").mode(0o644))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("etc/init.d/foo", TestFile::new("x").mode(0o765))]),
            BinaryPackageCase::from(vec![("etc/init.d/foo", TestFile::new("x").mode(0o644))]),
            // The permission bits include setuid.
            BinaryPackageCase::from(vec![("etc/init.d/foo", TestFile::new("x").mode(0o4755))]),
            // A hardlink is a file for lintian.
            BinaryPackageCase::from(vec![(
                "etc/init.d/foo",
                TestFile::new("x").hardlink().mode(0o600),
            )]),
            // Names that are not UTF-8 are matched on their bytes.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"etc/init.d/f\xffo".to_vec()),
                TestFile::new("x").mode(0o644),
            )]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("etc/init.d/foo", TestFile::new("x").mode(0o644))])
                .udeb(),
        ];
        BinaryPackageTester::new(NonStandardFilePermissionsForEtcInitDScript, pass, fail)
            .test_and_snapshot();
    }
}
