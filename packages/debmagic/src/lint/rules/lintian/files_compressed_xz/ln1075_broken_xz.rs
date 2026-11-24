use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct BrokenXz;

declare_rule! {
    /// A file whose name ends in `.xz` is not a valid xz stream.
    ///
    /// Walks the Data files of a Binary package, including udebs. Only regular
    /// files and hardlinks are checked; symlinks, fifos and devices are skipped.
    /// The name match is case-insensitive, as in lintian's `/\.xz$/si`. The
    /// content is decoded with xz's multi-stream decoder, so concatenated streams
    /// are accepted as `xz --test` accepts them; any decoding error, including
    /// an empty file or a failed block check, is reported. Extra is empty and
    /// the location is the path, with no line number.
    ///
    /// A file whose content cannot be read from the archive is skipped.
    BrokenXz,
    code = "LN1075",
    tag = "broken-xz",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BrokenXz {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut broken: Vec<PathBuf> = Vec::new();
        for entry in files.entries() {
            if !entry.is_file() || !has_xz_extension(entry.path()) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if !is_xz(&bytes) {
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

fn has_xz_extension(path: &Path) -> bool {
    path.as_os_str()
        .as_bytes()
        .to_ascii_lowercase()
        .ends_with(b".xz")
}

/// `xz --test`: the whole input decodes, with every block's check verified.
fn is_xz(bytes: &[u8]) -> bool {
    let mut decoder = xz2::read::XzDecoder::new_multi_decoder(bytes);
    io::copy(&mut decoder, &mut io::sink()).is_ok()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn xz_bytes(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 6);
        encoder.write_all(bytes).expect("xz");
        encoder.finish().expect("xz")
    }

    #[test]
    fn binary() {
        let good = xz_bytes(b"hello\n");
        let mut concatenated = xz_bytes(b"one\n");
        concatenated.extend(xz_bytes(b"two\n"));
        let mut truncated = xz_bytes(b"hello world, a bit longer than the header\n");
        truncated.truncate(truncated.len() / 2);
        let mut corrupt = xz_bytes(b"hello world, a bit longer than the header\n");
        let last = corrupt.len() - 1;
        corrupt[last] ^= 0xff;

        let pass = vec![
            BinaryPackageCase::from(vec![("etc/good.xz", TestFile::from_bytes(good.clone()))]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/concatenated.xz",
                TestFile::from_bytes(concatenated),
            )]),
            BinaryPackageCase::from(vec![("etc/plain.txt", TestFile::new("not compressed"))]),
            BinaryPackageCase::from(vec![("usr/lib/link.xz", TestFile::symlink("nowhere"))]),
            BinaryPackageCase::from(vec![("etc/good.xz", TestFile::from_bytes(good.clone()))])
                .control_file("preinst", TestFile::new("not compressed")),
            BinaryPackageCase::from(vec![("etc/good.xz", TestFile::from_bytes(good.clone()))])
                .udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("etc/not-an.xz", TestFile::new("not compressed"))]),
            BinaryPackageCase::from(vec![("etc/NOT-AN.XZ", TestFile::new("not compressed"))]),
            BinaryPackageCase::from(vec![("etc/empty.xz", TestFile::new(""))]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/truncated.xz",
                TestFile::from_bytes(truncated),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/corrupt.xz",
                TestFile::from_bytes(corrupt),
            )]),
            BinaryPackageCase::from(vec![(
                "etc/gzip-as-xz.xz",
                TestFile::from_bytes(b"\x1f\x8b\x08\x00".to_vec()),
            )]),
            BinaryPackageCase::from(vec![
                ("etc/a.xz", TestFile::new("not compressed")),
                (
                    "etc/b.xz",
                    TestFile::new("not compressed").hardlink_to("etc/a.xz"),
                ),
            ]),
            BinaryPackageCase::from(vec![("etc/not-an.xz", TestFile::new("not compressed"))])
                .udeb(),
        ];
        BinaryPackageTester::new(BrokenXz, pass, fail).test_and_snapshot();
    }
}
