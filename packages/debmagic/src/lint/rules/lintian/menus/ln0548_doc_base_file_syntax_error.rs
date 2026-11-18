use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseFileSyntaxError;

declare_rule! {
    /// A doc-base line is neither a field, a continuation, nor blank.
    ///
    /// An indented line before any field is also a syntax error. Location is
    /// the line. Extra is empty.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseFileSyntaxError,
    code = "LN0548",
    tag = "doc-base-file-syntax-error",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseFileSyntaxError {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::SyntaxError, false);
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
            doc("Document: example\n continued\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\n\nnot a field\n"),
            doc("  indented first\nDocument: example\n"),
            doc("Document: example\n\nnot a field\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseFileSyntaxError, pass, fail).test_and_snapshot();
    }
}
