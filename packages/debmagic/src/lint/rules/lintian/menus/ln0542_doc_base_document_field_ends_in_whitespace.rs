use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseDocumentFieldEndsInWhitespace;

declare_rule! {
    /// The `Document` value ends in a space or tab.
    ///
    /// Continuation lines are joined with a space first. Location is the
    /// field's last line. Extra is empty.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseDocumentFieldEndsInWhitespace,
    code = "LN0542",
    tag = "doc-base-document-field-ends-in-whitespace",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseDocumentFieldEndsInWhitespace {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::DocumentEndsInWhitespace, false);
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
            doc("Document: example\n"),
            doc("Document: example\n\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example \n"),
            doc("Document: example\t\n"),
            doc("Document: example \n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseDocumentFieldEndsInWhitespace, pass, fail)
            .test_and_snapshot();
    }
}
