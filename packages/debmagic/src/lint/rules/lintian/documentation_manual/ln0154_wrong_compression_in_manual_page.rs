use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::manual::{final_component_is_gz, is_file, is_gzip, section_page};

pub struct WrongCompressionInManualPage;

declare_rule! {
    /// A manual page named as gzip is not gzip data.
    ///
    /// A regular file or hardlink in a `usr/share/man/.../manN/` directory
    /// whose final `.`-separated component is `gz`, and whose bytes do not
    /// start with the gzip magic `1f 8b`. Symlinks are not read. A name with
    /// no dot uses the whole name, so a file named `gz` is judged, and Perl
    /// `split` keeps `foo.1.gz.` in this Tag. Udebs are skipped. Extra is
    /// empty; the Data files path is the Location.
    WrongCompressionInManualPage,
    code = "LN0154",
    tag = "wrong-compression-in-manual-page",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for WrongCompressionInManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            let Some(page) = section_page(entry) else {
                continue;
            };
            if !final_component_is_gz(page.file_name) || !is_file(entry) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if is_gzip(&bytes) {
                continue;
            }
            hits.push(entry.path().to_path_buf());
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

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn gzip(path: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::from_bytes([0x1f, 0x8b]))
    }

    fn plain(path: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new("not gzip"))
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![gzip("usr/share/man/man1/foo.1.gz")]),
            case(vec![gzip("usr/share/man/de/man1/foo.1.gz")]),
            // Symlinks are not read, even when the name ends in `.gz`.
            case(vec![(
                "usr/share/man/man1/pretend-compress.1.gz".to_string(),
                TestFile::symlink("missing.gz"),
            )]),
            // Uncompressed names belong to the other Tag.
            case(vec![plain("usr/share/man/man1/not-compressed.1")]),
            case(vec![plain("usr/share/man/man1/upper.1.GZ")]),
            case(vec![
                plain("usr/share/man/foo.1.gz"),
                plain("usr/share/man/odd/foo.1.gz"),
                plain("usr/bin/foo.1.gz"),
            ]),
            case(vec![plain("usr/share/man/man1/pretend-compress.1.gz")]).udeb(),
        ];
        let fail = vec![
            case(vec![plain("usr/share/man/man1/pretend-compress.1.gz")]),
            case(vec![plain("usr/share/man/man1/usr-bin-binary-alt.1.gz")]),
            case(vec![plain("usr/share/man/de/man1/foo.1.gz")]),
            case(vec![(
                "usr/share/man/man1/usr-bin-binary-alt.1.gz".to_string(),
                TestFile::new("not gzip").hardlink(),
            )]),
            case(vec![
                (
                    "usr/share/man/man1/empty.1.gz".to_string(),
                    TestFile::from_bytes([]),
                ),
                (
                    "usr/share/man/man1/short.1.gz".to_string(),
                    TestFile::from_bytes([0x1f]),
                ),
                (
                    "usr/share/man/man3/bad.3.gz".to_string(),
                    TestFile::from_bytes([0x1f, 0x8a]),
                ),
                plain("usr/share/man/man1/gz"),
                plain("usr/share/man/man1/foo.1.gz."),
            ]),
        ];
        BinaryPackageTester::new(WrongCompressionInManualPage, pass, fail).test_and_snapshot();
    }
}
