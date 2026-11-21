use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use flate2::read::MultiGzDecoder;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct BrokenGz;

declare_rule! {
    /// A file whose name ends in `.gz` (in any case) does not decompress as gzip.
    ///
    /// lintian runs `gzip --test` on every regular file and hardlink of the
    /// Data files whose name ends in `.gz`, and reports the path when gzip
    /// exits non-zero: the bytes are not gzip, the stream is truncated, a CRC
    /// or length check fails, or bytes follow the last member. Directories,
    /// symlinks and devices are not files and are skipped. Multi-member
    /// streams are valid. Extra is empty; the path is the Location.
    ///
    /// Differs from lintian: the test decodes in process with flate2 instead
    /// of running `gzip`. An entry whose bytes cannot be read is skipped.
    BrokenGz,
    code = "LN1069",
    tag = "broken-gz",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BrokenGz {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut broken: Vec<PathBuf> = Vec::new();
        for entry in files.entries() {
            if !entry.is_file() || !has_gz_suffix(entry.path().as_os_str().as_bytes()) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if !gzip_test_passes(&bytes) {
                broken.push(entry.path().to_path_buf());
            }
        }
        for path in broken {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// `$item->name =~ /\.gz$/si`.
fn has_gz_suffix(name: &[u8]) -> bool {
    name.len() >= 3 && name[name.len() - 3..].eq_ignore_ascii_case(b".gz")
}

/// Whether `gzip --test` accepts `bytes`. Empty input is rejected, as gzip does.
fn gzip_test_passes(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let mut plain = Vec::new();
    MultiGzDecoder::new(bytes).read_to_end(&mut plain).is_ok()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::io::Write;
    use std::os::unix::ffi::OsStringExt;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn gzip(plain: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(plain).expect("write");
        encoder.finish().expect("finish")
    }

    fn file(path: &str, content: Vec<u8>) -> (String, TestFile) {
        (path.to_string(), TestFile::from_bytes(content))
    }

    #[test]
    fn binary() {
        let good = gzip(b"hello\n");
        let mut multi = gzip(b"one\n");
        multi.extend(gzip(b"two\n"));
        let mut trailing = gzip(b"hello\n");
        trailing.extend(b"junkjunk");
        let truncated = good[..good.len() - 4].to_vec();

        let pass = vec![
            BinaryPackageCase::from(vec![file("etc/ok.gz", good.clone())]),
            BinaryPackageCase::from(vec![file("etc/multi.GZ", multi)]),
            // A name without the suffix is not checked.
            BinaryPackageCase::from(vec![file("etc/not-a.txt", b"plain".to_vec())]),
            // Hardlinks are files and pass when their content is gzip.
            BinaryPackageCase::from(vec![
                file("etc/target.gz", good.clone()),
                (
                    "etc/link.gz".to_string(),
                    TestFile::from_bytes(good.clone()).hardlink_to("etc/target.gz"),
                ),
            ]),
            // A symlink is not a file, and its name is not checked.
            BinaryPackageCase::from(vec![("etc/link.gz", TestFile::symlink("not-a.gz"))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![file("etc/not-a.gz", b"hello\n".to_vec())]),
            BinaryPackageCase::from(vec![file("etc/UPPER.Gz", b"hello\n".to_vec())]),
            BinaryPackageCase::from(vec![file("etc/empty.gz", Vec::new())]),
            BinaryPackageCase::from(vec![file("etc/truncated.gz", truncated)]),
            BinaryPackageCase::from(vec![file("etc/trailing.gz", trailing)]),
            // Non-UTF-8 name bytes are still checked.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"etc/\xff.gz".to_vec()),
                TestFile::from_bytes(b"hello\n".to_vec()),
            )]),
            // A udeb is checked like a deb.
            BinaryPackageCase::from(vec![file("etc/not-a.gz", b"hello\n".to_vec())]).udeb(),
            // A hardlink whose content is not gzip.
            BinaryPackageCase::from(vec![
                file("etc/target.gz", b"hello\n".to_vec()),
                (
                    "etc/link.gz".to_string(),
                    TestFile::from_bytes(b"hello\n".to_vec()).hardlink_to("etc/target.gz"),
                ),
            ]),
        ];
        BinaryPackageTester::new(BrokenGz, pass, fail).test_and_snapshot();
    }
}
