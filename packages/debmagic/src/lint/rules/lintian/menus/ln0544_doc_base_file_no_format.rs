use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseFileNoFormat;

declare_rule! {
    /// A section lists `Files` or `Index` but no `Format`.
    ///
    /// Checked when a section ends: the line after its last field, or its
    /// last line at end of file. Extra is empty.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseFileNoFormat,
    code = "LN0544",
    tag = "doc-base-file-no-format",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseFileNoFormat {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::NoFormat, false);
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
            doc("Document: example\n\nFormat: text\nIndex: /usr/share/doc/example/a\n"),
            doc("Document: example\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\n\nFiles: /usr/share/doc/example/a\n"),
            doc("Document: example\n\nIndex: /usr/share/doc/example/a\n\n"),
            doc("Document: example\n\nFiles: /usr/share/doc/example/a\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseFileNoFormat, pass, fail).test_and_snapshot();
    }
}
