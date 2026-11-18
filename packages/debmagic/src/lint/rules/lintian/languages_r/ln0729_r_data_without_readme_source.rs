use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;
use crate::lint::rules::lintian::debian_patches::ln0724_patch_system_but_no_source_readme::has_readme_source;

pub struct RDataWithoutReadmeSource;

declare_rule! {
    /// An R data file (`.rda`, `.Rda`, `.rdata`, `.Rdata` or `.RData`) whose
    /// gzip payload starts with the `RDX2` magic, in a tree without
    /// `debian/README.source`.
    ///
    /// Applies to the Source tree and Source package. Only regular files and
    /// hardlinks are judged. The file must start with the gzip magic
    /// (`1f 8b 08`), and only the first four bytes of its uncompressed payload
    /// are read. `debian/README.source` counts as in ln0724: a symlink is
    /// followed, and a dangling one does not count. Extra is empty. Location is
    /// the path, with no line number.
    RDataWithoutReadmeSource,
    code = "LN0729",
    tag = "r-data-without-readme-source",
    default_selected = true,
    experimental = false,
}

impl SourceRule for RDataWithoutReadmeSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        if has_readme_source(files) {
            return;
        }
        let hits: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| {
                matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                    && is_rdata_name(entry.path())
            })
            .filter(|entry| files.read(entry).is_ok_and(|bytes| is_rdx2_gzip(&bytes)))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn is_rdata_name(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| {
            [".rda", ".Rda", ".rdata", ".Rdata", ".RData"]
                .iter()
                .any(|suffix| name.ends_with(suffix))
        })
}

/// Gzip stream (deflate, as libmagic's `gzip compressed data`) whose first
/// four decompressed bytes are `RDX2`.
fn is_rdx2_gzip(bytes: &[u8]) -> bool {
    if !bytes.starts_with(&[0x1f, 0x8b, 0x08]) {
        return false;
    }
    let mut magic = [0u8; 4];
    flate2::read::GzDecoder::new(bytes)
        .read_exact(&mut magic)
        .is_ok()
        && &magic == b"RDX2"
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn gzip_bytes(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).expect("gzip");
        encoder.finish().expect("gzip")
    }

    fn rdata() -> TestFile {
        TestFile::from_bytes(gzip_bytes(b"RDX2\nX\n\0\0\0\x03"))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no r data"))],
            vec![("src/test.rda", TestFile::new("RDX2"))],
            vec![("src/test.rda", TestFile::from_bytes(gzip_bytes(b"RDX3\n")))],
            vec![("src/test.rda", TestFile::from_bytes(gzip_bytes(b"RD")))],
            vec![("src/test.rda", TestFile::from_bytes(b"\x1f\x8b\x08"))],
            vec![("src/test.RDA", rdata())],
            vec![("src/test.rds", rdata())],
            vec![("src/test.RDATA", rdata())],
            vec![("src/rda", rdata())],
            vec![("src/test.rda", TestFile::directory())],
            vec![("src/test.rda", TestFile::symlink("other"))],
            vec![
                ("src/test.rda", rdata()),
                ("debian/README.source", TestFile::new("explained\n")),
            ],
            vec![
                ("src/test.rda", rdata()),
                ("debian/README.source", TestFile::symlink("../README")),
                ("README", TestFile::new("explained\n")),
            ],
        ];
        let fail = vec![
            vec![("src/test.rda", rdata())],
            vec![("src/test.Rda", rdata())],
            vec![("src/test.rdata", rdata())],
            vec![("src/test.Rdata", rdata())],
            vec![("src/test.RData", rdata())],
            vec![("src/test.rda", rdata().hardlink())],
            vec![("src/test.rda", rdata()), ("src/other.rda", rdata())],
            vec![
                ("src/test.rda", rdata()),
                ("debian/README.source", TestFile::symlink("missing")),
            ],
            vec![
                ("src/test.rda", rdata()),
                ("debian/README", TestFile::new("explained\n")),
            ],
        ];
        SourceTreeTester::new(RDataWithoutReadmeSource, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "src/test.rda",
            TestFile::new("RDX2"),
        )])];
        let fail = vec![SourcePackageCase::from(vec![("src/test.rda", rdata())])];
        SourcePackageTester::new(RDataWithoutReadmeSource, pass, fail).test_and_snapshot();
    }
}
