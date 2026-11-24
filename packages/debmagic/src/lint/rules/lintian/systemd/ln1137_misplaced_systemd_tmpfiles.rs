use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct MisplacedSystemdTmpfiles;

declare_rule! {
    /// A regular file or hardlink of the Binary package sits at
    /// `etc/tmpfiles.d/*.conf`, outside `/usr/lib/tmpfiles.d/`, where
    /// systemd's tmpfiles.d(5) configuration belongs.
    ///
    /// Only the Data files are read; a directory, symlink, fifo or device
    /// named like that is not reported. The match is lintian's regex on the
    /// raw path bytes, so a non-UTF-8 name is matched too. Extra is empty and
    /// Location is the path of the file, with no line.
    MisplacedSystemdTmpfiles,
    code = "LN1137",
    tag = "misplaced-systemd-tmpfiles",
    default_selected = true,
    experimental = false,
}

/// lintian's `m{^ etc/tmpfiles[.]d/ .* [.]conf $}sx`, on raw path bytes.
/// Perl's `$` also matches before a final newline, hence the optional `\n`.
static MISPLACED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s-u)^etc/tmpfiles\.d/.*\.conf\n?$").expect("valid regex"));

impl BinaryPackageRule for MisplacedSystemdTmpfiles {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_file())
            .map(|entry| entry.path().to_path_buf())
            .filter(|path| MISPLACED.is_match(path.as_os_str().as_encoded_bytes()))
            .collect();

        for path in paths {
            ctx.diagnostic(Diagnostic::error(String::new()).with_location(Location {
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

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestCase, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    #[test]
    fn binary() {
        let pass = vec![
            data("usr/lib/tmpfiles.d/test.conf", TestFile::new("d /run/x\n")),
            // Only a conf file in etc/tmpfiles.d/ is reported.
            data("etc/tmpfiles.d/test.txt", TestFile::new("d /run/x\n")),
            data("etc/tmpfiles.d/test.conf.bak", TestFile::new("d /run/x\n")),
            data("etc/other/test.conf", TestFile::new("d /run/x\n")),
            // Not a file: a directory, a symlink, a fifo or a device.
            data("etc/tmpfiles.d/dir.conf", TestFile::directory()),
            data("etc/tmpfiles.d/link.conf", TestFile::symlink("/dev/null")),
            data("etc/tmpfiles.d/pipe.conf", TestFile::fifo()),
            data("etc/tmpfiles.d/null.conf", TestFile::char_device()),
            // Only a path that starts with etc/tmpfiles.d/ is matched.
            data(
                "usr/lib/tmpfiles.d/etc/tmpfiles.d/x.conf",
                TestFile::new(""),
            ),
        ];
        let fail = vec![
            data("etc/tmpfiles.d/test.conf", TestFile::new("d /run/x\n")),
            // A hardlink is a file.
            data(
                "etc/tmpfiles.d/test.conf",
                TestFile::new("d /run/x\n").hardlink(),
            ),
            // Nested under a subdirectory, and an empty stem.
            data("etc/tmpfiles.d/sub/test.conf", TestFile::new("")),
            data("etc/tmpfiles.d/.conf", TestFile::new("")),
            // A non-UTF-8 name is matched on its raw bytes.
            BinaryPackageCase::from(TestCase::source_tree(vec![(
                OsString::from_vec(b"etc/tmpfiles.d/\xff.conf".to_vec()),
                TestFile::new(""),
            )])),
        ];
        BinaryPackageTester::new(MisplacedSystemdTmpfiles, pass, fail).test_and_snapshot();
    }
}
