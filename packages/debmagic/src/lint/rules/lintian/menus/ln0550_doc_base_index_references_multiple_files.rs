use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseIndexReferencesMultipleFiles;

declare_rule! {
    /// An `Index` field lists more than one file.
    ///
    /// Entries are whitespace-separated across the field's lines. Location is
    /// the field's last line. Extra is empty.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseIndexReferencesMultipleFiles,
    code = "LN0550",
    tag = "doc-base-index-references-multiple-files",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseIndexReferencesMultipleFiles {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::IndexReferencesMultipleFiles, false);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn doc(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/doc-base/example", TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            doc(
                "Document: example\n\nFormat: html\nIndex: /usr/share/doc/example/index.html\nFiles: /usr/share/doc/example/a /usr/share/doc/example/b\n",
            ),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\n\nFormat: html\nIndex: /usr/share/doc/example/index.html /usr/share/doc/example/b.html\n"),
            doc("Document: example\n\nFormat: html\nIndex: /usr/share/doc/example/index.html  \n /usr/share/doc/example/ch1.html\n"),
            doc("Document: example\n\nFormat: html\nIndex: /usr/share/doc/example/index.html /usr/share/doc/example/b.html\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseIndexReferencesMultipleFiles, pass, fail)
            .test_and_snapshot();
    }
}
