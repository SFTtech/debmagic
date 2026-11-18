use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct InfoDocumentNotCompressedWithGzip;

declare_rule! {
    /// An info document whose name ends in `.gz` is a regular file or hardlink
    /// that does not start with the gzip magic bytes. Symlinks are skipped, as
    /// are `dir` files. Extra is empty.
    InfoDocumentNotCompressedWithGzip,
    code = "LN0749",
    tag = "info-document-not-compressed-with-gzip",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InfoDocumentNotCompressedWithGzip {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let bad: Vec<PathBuf> = scan::info_items(files)
            .filter(|entry| {
                if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                    return false;
                }
                let basename = scan::basename(entry);
                if scan::name_pieces(&basename).last() != Some(&"gz") {
                    return false;
                }
                files.read(entry).is_ok_and(|bytes| !scan::is_gzip(&bytes))
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in bad {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn file(name: &str, content: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(format!("usr/share/info/{name}"), content)])
    }

    #[test]
    fn test() {
        let pass = vec![
            file("foo.info.gz", gzip("text\n")),
            file("foo.info", TestFile::new("text\n")),
            file("foo.info-1.gz", gzip("text\n")),
            file("foo.png.gz", gzip("text\n")),
            file("foo.info.gz", TestFile::symlink("bar.info.gz")),
            file("dir.gz", TestFile::new("text\n")),
        ];
        let fail = vec![
            file("foo.info.gz", TestFile::new("text\n")),
            file("foo.gz", TestFile::new("")),
            file("foo.info-1.gz", TestFile::new("\x1fx")),
        ];
        BinaryPackageTester::new(InfoDocumentNotCompressedWithGzip, pass, fail).test_and_snapshot();
    }
}
