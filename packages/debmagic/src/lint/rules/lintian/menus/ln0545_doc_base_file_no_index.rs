use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseFileNoIndex;

declare_rule! {
    /// An `html` or `info` format section has no `Index` field.
    ///
    /// The format is the section's last `Format`, lower-cased. Checked when a
    /// section ends: the line after its last field, or its last line at end of
    /// file. Extra is empty.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseFileNoIndex,
    code = "LN0545",
    tag = "doc-base-file-no-index",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseFileNoIndex {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::NoIndex, false);
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
                "Document: example\n\nFormat: html\nIndex: /usr/share/doc/example/index.html\nFiles: /usr/share/doc/example/*.html\n",
            ),
            doc("Document: example\n\nFormat: text\nFiles: /usr/share/doc/example/a\n"),
            doc("Document: example\n\nFiles: /usr/share/doc/example/a\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\n\nFormat: HTML\nFiles: /usr/share/doc/example/*.html\n"),
            doc("Document: example\n\nFormat: info\nFiles: /usr/share/info/example.info.gz\n\n"),
            doc("Document: example\n\nFormat: HTML\nFiles: /usr/share/doc/example/*.html\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseFileNoIndex, pass, fail).test_and_snapshot();
    }
}
