use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::zip::ZipArchive;

pub struct BrokenZip;

declare_rule! {
    /// A file whose name ends in `.zip` does not read as a zip archive.
    ///
    /// Walks the regular files and hardlinks in Data files whose name ends in
    /// `.zip` (in any letter case). lintian runs `unzip -l` and reports when it
    /// exits non-zero; this Rule reads the central directory with this crate's
    /// own zip reader (`lint::zip`) and reports when that fails. Only the
    /// central directory is read, as `unzip -l` does, so members are not
    /// decompressed. Extra is empty; the file is the Location.
    ///
    /// Differences: the reader is not `unzip`, so a few corrupt archives that
    /// `unzip -l` lists may be reported, and vice versa. Encrypted members are
    /// not checked (lintian does not check them either). A file that cannot be
    /// read is skipped.
    BrokenZip,
    code = "LN1251",
    tag = "broken-zip",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BrokenZip {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let broken = broken_zips(files);
        for path in broken {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn is_zip_name(entry: &Entry) -> bool {
    entry
        .path()
        .as_os_str()
        .as_bytes()
        .to_ascii_lowercase()
        .ends_with(b".zip")
}

fn broken_zips(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| is_zip_name(entry))
        .filter(|entry| match files.read(entry) {
            Ok(bytes) => {
                let name = entry.path().to_string_lossy();
                ZipArchive::parse(&bytes, &name).is_err()
            }
            Err(_) => false,
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::jar::TestJar;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(path: &str, bytes: Vec<u8>) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(bytes))])
    }

    #[test]
    fn binary() {
        let valid = TestJar::new().class("a/A.class", 52).build();
        let truncated = valid[..valid.len() - 25].to_vec();
        let pass = vec![
            case("usr/share/java/a.zip", valid.clone()),
            // Not named `.zip`.
            case("usr/share/java/a.jar", b"not a zip".to_vec()),
        ];
        let fail = vec![
            case("etc/not-a.zip", b"not a zip".to_vec()),
            case("etc/empty.zip", Vec::new()),
            case("etc/truncated.zip", truncated.clone()),
            case("etc/TRUNCATED.ZIP", truncated.clone()),
            case("etc/PK.zip", b"PK\x03\x04 tiny".to_vec()),
            BinaryPackageCase::from(vec![(
                "etc/hard.zip",
                TestFile::from_bytes(truncated).hardlink(),
            )]),
        ];
        BinaryPackageTester::new(BrokenZip, pass, fail).test_and_snapshot();
    }
}
