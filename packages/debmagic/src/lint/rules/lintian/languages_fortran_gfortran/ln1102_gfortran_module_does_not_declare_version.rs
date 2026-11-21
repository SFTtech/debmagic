use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use flate2::read::GzDecoder;
use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct GfortranModuleDoesNotDeclareVersion;

declare_rule! {
    /// A gzip-compressed `.mod` file under `usr/lib/` does not declare a
    /// GFORTRAN module version on its first non-blank line
    /// (`GFORTRAN module version '<n>'`), so binary rebuilds cannot establish
    /// the right prerequisites.
    ///
    /// Only regular files and hardlinks are read. The gzip magic is sniffed
    /// (`1f 8b 08`) in place of `file(1)`. Paths under `flang-<n>/`,
    /// `usr/lib/grub` and `usr/lib/libreoffice` are skipped. The gzip stream
    /// is read only up to the first non-blank line, as lintian reads it, so a
    /// corrupt tail after it is not an error. Extra is empty; the module path
    /// is the Location. The companion
    /// `missing-prerequisite-for-gfortran-module` (LN1103) reads the same
    /// files.
    GfortranModuleDoesNotDeclareVersion,
    code = "LN1102",
    tag = "gfortran-module-does-not-declare-version",
    default_selected = true,
    experimental = false,
}

/// A gzip-compressed Fortran module of the Data files, and the version its
/// first non-blank line declares, if any.
pub(super) struct GfortranModule {
    pub(super) path: PathBuf,
    pub(super) version: Option<String>,
}

static MODULE_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)^usr/lib/.*\.mod$").expect("valid regex"));

static SKIPPED_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u)/flang-[0-9]+/|^usr/lib/grub|^usr/lib/libreoffice").expect("valid regex")
});

static VERSION_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)^GFORTRAN module version '([0-9]+)'").expect("valid regex"));

/// Perl's `\s`, as lintian tests blank lines with it.
const BLANK: &[u8] = b" \t\r\n\x0b\x0c";

impl BinaryPackageRule for GfortranModuleDoesNotDeclareVersion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let undeclared: Vec<PathBuf> = gfortran_modules(files)
            .into_iter()
            .filter(|module| module.version.is_none())
            .map(|module| module.path)
            .collect();
        for path in undeclared {
            ctx.diagnostic(Diagnostic::warn("").with_location(at(&path)));
        }
    }
}

/// The gzip-compressed Fortran modules of a File tree, in file order.
pub(super) fn gfortran_modules(files: &FileTree) -> Vec<GfortranModule> {
    files
        .entries()
        .iter()
        .filter(|entry| entry.is_file())
        .filter(|entry| {
            let name = entry.path().as_os_str().as_encoded_bytes();
            MODULE_PATH.is_match(name) && !SKIPPED_PATH.is_match(name)
        })
        .filter_map(|entry| {
            // An unreadable file has no bytes to judge: skip it.
            let bytes = files.read(entry).ok()?;
            if !bytes.starts_with(&[0x1f, 0x8b, 0x08]) {
                return None;
            }
            Some(GfortranModule {
                path: entry.path().to_path_buf(),
                version: declared_version(&bytes),
            })
        })
        .collect()
}

/// The version on the first non-blank line of the gzip stream, if that line
/// declares one. Anything that stops the read before that line is no version.
fn declared_version(gzipped: &[u8]) -> Option<String> {
    let mut reader = BufReader::new(GzDecoder::new(gzipped));
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return None,
            Ok(_) => {}
        }
        if line.iter().all(|byte| BLANK.contains(byte)) {
            continue;
        }
        let caps = VERSION_LINE.captures(&line)?;
        return Some(String::from_utf8_lossy(&caps[1]).into_owned());
    }
}

pub(super) fn at(path: &Path) -> Location {
    Location {
        path: path.to_path_buf(),
        line: None,
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::io::Write;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn module(path: &str, text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), gzip(text))])
    }

    #[test]
    fn binary() {
        let declared = "GFORTRAN module version '14' created from mpi.f90\n";
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            module("usr/lib/fortran/mpi.mod", declared),
            // Blank lines first are skipped.
            module("usr/lib/fortran/mpi.mod", &format!("\n \t\n{declared}")),
            // Not gzip-compressed: `file(1)` would not say so either.
            BinaryPackageCase::from(vec![(
                "usr/lib/fortran/mpi.mod",
                TestFile::new("GFORTRAN module version '14'\n"),
            )]),
            // Outside `usr/lib/`, or not a `.mod`.
            module("usr/share/fortran/mpi.mod", "no version\n"),
            module("usr/lib/fortran/mpi.so", "no version\n"),
            // Skipped subtrees.
            module("usr/lib/flang-14/mpi.mod", "no version\n"),
            module("usr/lib/grub/x86/fortran.mod", "no version\n"),
            module("usr/lib/libreoffice/program/fortran.mod", "no version\n"),
            // Not a file.
            BinaryPackageCase::from(vec![(
                "usr/lib/fortran/mpi.mod",
                TestFile::symlink("/usr/lib/fortran/other.mod"),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/fortran/mpi.mod", TestFile::directory())]),
        ];
        let fail = vec![
            module("usr/lib/fortran/mpi.mod", "no version here\n"),
            // The version must be the first non-blank line.
            module(
                "usr/lib/fortran/mpi.mod",
                &format!("some preamble\n{declared}"),
            ),
            // Only the first non-blank line counts, and an empty stream has none.
            module("usr/lib/fortran/mpi.mod", ""),
            module("usr/lib/fortran/mpi.mod", "\n\n"),
            // A hardlink is a file for lintian.
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/fortran/mpi.mod",
                    gzip("no version\n").hardlink_to("usr/lib/fortran/other.mod"),
                ),
                ("usr/lib/fortran/other.mod", gzip("no version\n")),
            ]),
            // Udebs are checked too.
            module("usr/lib/fortran/mpi.mod", "no version\n").udeb(),
            // A name that is not UTF-8 is still judged.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/lib/fortran/bokm\xe5l.mod".to_vec()),
                gzip("no version\n"),
            )]),
        ];
        BinaryPackageTester::new(GfortranModuleDoesNotDeclareVersion, pass, fail)
            .test_and_snapshot();
    }
}
