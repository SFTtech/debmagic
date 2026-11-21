use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PythonDebugInWrongLocation;

/// Debug files for a Python module installed under `pyshared` rather than
/// `pymodules`. Group 1 is `pythonX.Y` (the dot part is required), group 2 is
/// the rest of the path. `(?-u:...)` keeps `.` and `.+` byte-based, so names
/// that are not UTF-8 still match.
static PYSHARED_DEBUG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/debug/usr/lib/pyshared/(python(?-u:[0-9])?(?:\.[0-9]+))/((?-u:.+))$")
        .expect("valid regex")
});

declare_rule! {
    /// A Python debug file is installed under `usr/lib/debug/usr/lib/pyshared/`.
    ///
    /// Each regular file or hardlink matching
    /// `usr/lib/debug/usr/lib/pyshared/pythonX.Y/...` is reported with its path
    /// as the Location, and the extra `better: <path>` naming the
    /// `usr/lib/debug/usr/lib/pymodules/` location. Directories and symlinks
    /// are skipped. The extra is built with lossy UTF-8 conversion, since a
    /// hint message is text; the Location keeps the raw path. Udebs are
    /// checked too.
    PythonDebugInWrongLocation,
    code = "LN1116",
    tag = "python-debug-in-wrong-location",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PythonDebugInWrongLocation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let reports: Vec<(PathBuf, String)> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory && entry.is_file())
            .filter_map(|entry| {
                let bytes = entry.path().as_os_str().as_encoded_bytes();
                let captures = PYSHARED_DEBUG.captures(bytes)?;
                let mut correct = b"usr/lib/debug/usr/lib/pymodules/".to_vec();
                correct.extend_from_slice(&captures[1]);
                correct.push(b'/');
                correct.extend_from_slice(&captures[2]);
                Some((
                    entry.path().to_path_buf(),
                    format!("better: {}", String::from_utf8_lossy(&correct)),
                ))
            })
            .collect();
        for (path, extra) in reports {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
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

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(vec![
                // Already in the right place.
                (
                    "usr/lib/debug/usr/lib/pymodules/python2.7/python-module.so".to_string(),
                    TestFile::new("x"),
                ),
                // A directory is not reported.
                (
                    "usr/lib/debug/usr/lib/pyshared/python2.7/python-module.so".to_string(),
                    TestFile::directory(),
                ),
                // No dot in the version, so the path does not match.
                (
                    "usr/lib/debug/usr/lib/pyshared/python2/python-module.so".to_string(),
                    TestFile::new("x"),
                ),
                // Nothing below the version directory.
                (
                    "usr/lib/debug/usr/lib/pyshared/python2.7/".to_string(),
                    TestFile::directory(),
                ),
            ]),
            // A symlink is not a file.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/lib/pyshared/python2.7/python-module.so".to_string(),
                TestFile::symlink("elsewhere"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/lib/pyshared/python2.7/python-module.so".to_string(),
                TestFile::new("x"),
            )]),
            // The rest of the path is kept, and a hardlink counts as a file.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/lib/pyshared/python3.12/sub/mod.so".to_string(),
                TestFile::new("x").hardlink(),
            )]),
            // Udeb packages are checked as well.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/lib/pyshared/python3.11/mod.so".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PythonDebugInWrongLocation, pass, fail).test_and_snapshot();
    }
}
