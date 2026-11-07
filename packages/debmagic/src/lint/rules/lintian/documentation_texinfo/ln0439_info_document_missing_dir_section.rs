use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct InfoDocumentMissingDirSection;

declare_rule! {
    /// A gzip info document has no `INFO-DIR-SECTION` line with a section name.
    ///
    /// Descendants of `usr/share/info` other than `dir` files. Numbered splits
    /// (`*-N.gz`) are skipped. This is the `.deb` check, so udebs are not
    /// checked. Extra is empty.
    InfoDocumentMissingDirSection,
    code = "LN0439",
    tag = "info-document-missing-dir-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InfoDocumentMissingDirSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let documents = scan::info_documents(files);
        for document in documents {
            if !document.missing_section {
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

    #[test]
    fn test() {
        let pass = vec![
            info("foo.info.gz", "INFO-DIR-SECTION Software\n"),
            info("foo.info-1.gz", "no section\n"),
            info("dir.old.gz", "no section\n"),
        ];
        let fail = vec![
            info("bar.info.gz", "plain\n"),
            info("bar.info.gz", "INFO-DIR-SECTION\n"),
            info("bar.newinfo.gz", "INFO-DIR-SECTION   \n"),
        ];
        BinaryPackageTester::new(InfoDocumentMissingDirSection, pass, fail).test_and_snapshot();
    }
}
