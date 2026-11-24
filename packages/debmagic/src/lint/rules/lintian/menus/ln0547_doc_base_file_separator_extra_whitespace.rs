use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseFileSeparatorExtraWhitespace;

declare_rule! {
    /// A blank separator line between doc-base sections holds whitespace.
    ///
    /// Every whitespace-only line counts, also the repeated ones. Location is
    /// that line. Extra is empty.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseFileSeparatorExtraWhitespace,
    code = "LN0547",
    tag = "doc-base-file-separator-extra-whitespace",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseFileSeparatorExtraWhitespace {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::FileSeparatorExtraWhitespace, true);
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
                "Document: example\nTitle: T\nSection: Help\n\nFormat: text\nFiles: /usr/share/doc/example/a\n",
            ),
            doc("Document: example\n\n\nFormat: text\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\n \nFormat: text\n"),
            doc("Document: example\n\n  \n\nFormat: text\n"),
            doc("Document: example\n \nFormat: text\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseFileSeparatorExtraWhitespace, pass, fail)
            .test_and_snapshot();
    }
}
