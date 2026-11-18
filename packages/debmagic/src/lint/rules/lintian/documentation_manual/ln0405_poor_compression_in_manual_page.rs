use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::manual::{final_component_is_gz, is_file, is_gzip, section_page};

pub struct PoorCompressionInManualPage;

declare_rule! {
    /// A gzip manual page was not compressed with maximum compression.
    ///
    /// A regular file or hardlink in a `usr/share/man/.../manN/` directory
    /// whose final `.`-separated component is `gz`, whose bytes start with
    /// the gzip magic, and whose XFL byte (header offset 8) is not `2`.
    /// Symlinks are not read. Udebs are skipped. Extra is empty. The Location
    /// is the path.
    PoorCompressionInManualPage,
    code = "LN0405",
    tag = "poor-compression-in-manual-page",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PoorCompressionInManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            if !is_file(entry) {
                continue;
            }
            let Some(page) = section_page(entry) else {
                continue;
            };
            if !final_component_is_gz(page.file_name) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if is_gzip(&bytes) && !max_compression(&bytes) {
                hits.push(entry.path().to_path_buf());
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Gzip XFL `2` is what `file` reports as max compression. The byte sits at
/// offset 8 of the 10-byte header, before any name or comment.
fn max_compression(bytes: &[u8]) -> bool {
    bytes.len() >= 10 && bytes[8] == 2
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn gzip(xfl: u8) -> TestFile {
        let mut bytes = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, xfl, 3];
        bytes.extend_from_slice(b"payload");
        TestFile::from_bytes(bytes)
    }

    fn page(name: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(name.to_string(), file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            page("usr/share/man/man1/foo.1.gz", gzip(2)),
            page("usr/share/man/man1/foo.1", gzip(0)),
            page(
                "usr/share/man/man6/not-gzip.6.gz",
                TestFile::from_bytes(b"not gzip".to_vec()),
            ),
            page("usr/share/man/man1/foo.1.gz", gzip(0)).udeb(),
        ];
        let fail = vec![
            page("usr/share/man/man6/not-max-compress.6.gz", gzip(0)),
            page("usr/share/man/man6/usr-games-binary.6.gz", gzip(4)),
            page("usr/share/man/man1/foo.1.gz", gzip(0).hardlink()),
        ];
        BinaryPackageTester::new(PoorCompressionInManualPage, pass, fail).test_and_snapshot();
    }
}
