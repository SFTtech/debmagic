use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseDocumentFieldNotInFirstLine;

declare_rule! {
    /// The `Document` field does not end on the first line of the file.
    ///
    /// Compared with the field's last line, so a wrapped `Document` also
    /// counts. Location is that line. Extra is empty.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseDocumentFieldNotInFirstLine,
    code = "LN0543",
    tag = "doc-base-document-field-not-in-first-line",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseDocumentFieldNotInFirstLine {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::DocumentNotInFirstLine, false);
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
            doc("Document: example\nTitle: T\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Title: T\nDocument: example\n"),
            doc("Document: example\n continued\n"),
            doc("Title: T\nDocument: example\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseDocumentFieldNotInFirstLine, pass, fail)
            .test_and_snapshot();
    }
}
