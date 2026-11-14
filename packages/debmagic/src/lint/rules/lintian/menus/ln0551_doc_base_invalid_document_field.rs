use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseInvalidDocumentField;

declare_rule! {
    /// The `Document` value is not lower-case letters, digits, and `+.-`.
    ///
    /// Continuation lines are joined with a space first. Extra is the value.
    /// Location is the field's last line.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseInvalidDocumentField,
    code = "LN0551",
    tag = "doc-base-invalid-document-field",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseInvalidDocumentField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::InvalidDocument, false);
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
            doc("Document: example+1.x-y\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: Bad_Name\n"),
            doc("Document: two\n words\n"),
            doc("Document: Bad_Name\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseInvalidDocumentField, pass, fail).test_and_snapshot();
    }
}
