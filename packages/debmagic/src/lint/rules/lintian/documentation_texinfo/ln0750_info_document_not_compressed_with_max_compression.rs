use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct InfoDocumentNotCompressedWithMaxCompression;

declare_rule! {
    /// An info document whose name ends in `.gz` and that is gzip data does not
    /// record maximum compression in its gzip header (the XFL byte is not 2,
    /// as `gzip -9` writes it). Symlinks are skipped, as are `dir` files.
    /// Extra is empty.
    InfoDocumentNotCompressedWithMaxCompression,
    code = "LN0750",
    tag = "info-document-not-compressed-with-max-compression",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InfoDocumentNotCompressedWithMaxCompression {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let weak: Vec<PathBuf> = scan::info_items(files)
            .filter(|entry| {
                if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                    return false;
                }
                let basename = scan::basename(entry);
                if scan::name_pieces(&basename).last() != Some(&"gz") {
                    return false;
                }
                files
                    .read(entry)
                    .is_ok_and(|bytes| scan::is_gzip(&bytes) && !scan::is_max_compression(&bytes))
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in weak {
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

    fn gzip(text: &str, level: flate2::Compression) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), level);
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn file(name: &str, content: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(format!("usr/share/info/{name}"), content)])
    }

    #[test]
    fn test() {
        let pass = vec![
            file("foo.info.gz", gzip("text\n", flate2::Compression::best())),
            file("foo.info-1.gz", gzip("text\n", flate2::Compression::best())),
            file("foo.info", gzip("text\n", flate2::Compression::fast())),
            file("foo.info.gz", TestFile::new("text\n")),
            file("foo.info.gz", TestFile::symlink("bar.info.gz")),
            file("dir.gz", gzip("text\n", flate2::Compression::fast())),
        ];
        let fail = vec![
            file("foo.info.gz", gzip("text\n", flate2::Compression::fast())),
            file(
                "foo.info-1.gz",
                gzip("text\n", flate2::Compression::default()),
            ),
        ];
        BinaryPackageTester::new(InfoDocumentNotCompressedWithMaxCompression, pass, fail)
            .test_and_snapshot();
    }
}
