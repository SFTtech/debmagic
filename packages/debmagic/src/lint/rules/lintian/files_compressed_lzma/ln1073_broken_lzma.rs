use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use xz2::stream::Stream;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct BrokenLzma;

declare_rule! {
    /// A file whose name ends in `.lzma` is not a valid lzma (`lzma_alone`) stream.
    ///
    /// Walks the Data files of a Binary package, including udebs. Only regular
    /// files and hardlinks are checked; symlinks, fifos and devices are skipped.
    /// The name match is case-insensitive, as in lintian's `/\.lzma$/si`. The
    /// content is decoded as `lzma --test` does, and any decoding error,
    /// including an empty file, is reported. Extra is empty and the location is
    /// the path, with no line number.
    ///
    /// A file whose content cannot be read from the archive is skipped.
    BrokenLzma,
    code = "LN1073",
    tag = "broken-lzma",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BrokenLzma {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut broken: Vec<PathBuf> = Vec::new();
        for entry in files.entries() {
            if !entry.is_file() || !has_lzma_extension(entry.path()) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if !is_lzma(&bytes) {
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

fn has_lzma_extension(path: &Path) -> bool {
    path.as_os_str()
        .as_bytes()
        .to_ascii_lowercase()
        .ends_with(b".lzma")
}

/// `lzma --test`: the whole input decodes as an lzma_alone stream.
fn is_lzma(bytes: &[u8]) -> bool {
    let Ok(stream) = Stream::new_lzma_decoder(u64::MAX) else {
        return false;
    };
    let mut decoder = xz2::read::XzDecoder::new_stream(bytes, stream);
    io::copy(&mut decoder, &mut io::sink()).is_ok()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use xz2::stream::{LzmaOptions, Stream};

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn lzma_bytes(bytes: &[u8]) -> Vec<u8> {
        let options = LzmaOptions::new_preset(6).expect("preset");
        let stream = Stream::new_lzma_encoder(&options).expect("encoder");
        let mut encoder = xz2::write::XzEncoder::new_stream(Vec::new(), stream);
        encoder.write_all(bytes).expect("lzma");
        encoder.finish().expect("lzma")
    }

    #[test]
    fn binary() {
        let good = lzma_bytes(b"hello\n");
        let mut truncated = lzma_bytes(b"hello world, a bit longer than the header\n");
        truncated.truncate(truncated.len() / 2);

        let pass = vec![
            BinaryPackageCase::from(vec![("etc/good.lzma", TestFile::from_bytes(good.clone()))]),
            BinaryPackageCase::from(vec![("etc/plain.txt", TestFile::new("not compressed"))]),
            BinaryPackageCase::from(vec![("usr/lib/link.lzma", TestFile::symlink("nowhere"))]),
            BinaryPackageCase::from(vec![("etc/good.lzma", TestFile::from_bytes(good.clone()))])
                .control_file("preinst", TestFile::new("not compressed")),
            BinaryPackageCase::from(vec![("etc/good.lzma", TestFile::from_bytes(good.clone()))])
                .udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("etc/not-an.lzma", TestFile::new("not compressed"))]),
            BinaryPackageCase::from(vec![("etc/NOT-AN.LZMA", TestFile::new("not compressed"))]),
            BinaryPackageCase::from(vec![("etc/empty.lzma", TestFile::new(""))]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/truncated.lzma",
                TestFile::from_bytes(truncated),
            )]),
            BinaryPackageCase::from(vec![(
                "etc/xz-as-lzma.lzma",
                TestFile::from_bytes(b"\xfd7zXZ\x00".to_vec()),
            )]),
            BinaryPackageCase::from(vec![("etc/not-an.lzma", TestFile::new("not compressed"))])
                .udeb(),
        ];
        BinaryPackageTester::new(BrokenLzma, pass, fail).test_and_snapshot();
    }
}
