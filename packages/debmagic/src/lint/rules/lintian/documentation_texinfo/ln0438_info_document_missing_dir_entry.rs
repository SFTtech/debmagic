use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct InfoDocumentMissingDirEntry;

declare_rule! {
    /// A gzip info document has no `START-INFO-DIR-ENTRY` / `END-INFO-DIR-ENTRY` pair.
    ///
    /// Descendants of `usr/share/info` other than `dir` files. Numbered splits
    /// (`*-N.gz`) are skipped. This is the `.deb` check, so udebs are not
    /// checked. Extra is empty.
    InfoDocumentMissingDirEntry,
    code = "LN0438",
    tag = "info-document-missing-dir-entry",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InfoDocumentMissingDirEntry {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let documents = scan::info_documents(files);
        for document in documents {
            if !document.missing_entry {
                continue;
            }
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: document.path,
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
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn info(name: &str, text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(format!("usr/share/info/{name}"), gzip(text))])
    }

    const COMPLETE: &str =
        "INFO-DIR-SECTION Development\nSTART-INFO-DIR-ENTRY\n* foo: (foo).\nEND-INFO-DIR-ENTRY\n";

    #[test]
    fn test() {
        let pass = vec![
            info("foo.info.gz", COMPLETE),
            info("foo.info-1.gz", "no entry\n"),
            info("dir.gz", "no entry\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/info/foo.info.gz",
                TestFile::new("not gzip"),
            )]),
            info("bar.info.gz", "no entry\n").udeb(),
        ];
        let fail = vec![
            info("bar.info.gz", "plain info\n"),
            info("bar.info.gz", "START-INFO-DIR-ENTRY\n* foo\n"),
            info("bar.newinfo.gz", "END-INFO-DIR-ENTRY\n"),
        ];
        BinaryPackageTester::new(InfoDocumentMissingDirEntry, pass, fail).test_and_snapshot();
    }
}
