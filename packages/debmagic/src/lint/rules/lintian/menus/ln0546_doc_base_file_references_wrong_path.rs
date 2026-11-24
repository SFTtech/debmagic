use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseFileReferencesWrongPath;

declare_rule! {
    /// An `Index` or `Files` entry is outside `/usr/share/doc/` and `/usr/share/info/`.
    ///
    /// Entries are whitespace-separated across the field's lines. Extra is
    /// the entry. Location is the field's last line.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseFileReferencesWrongPath,
    code = "LN0546",
    tag = "doc-base-file-references-wrong-path",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseFileReferencesWrongPath {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::ReferencesWrongPath, false);
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
            doc("Document: example\n\nFormat: info\nIndex: /usr/share/info/example.info.gz\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\n\nFormat: text\nFiles: /opt/example/a\n"),
            doc(
                "Document: example\n\nFormat: text\nFiles: /usr/share/doc/example/a /usr/share/man/x\n /tmp/y\n",
            ),
            doc("Document: example\n\nFormat: text\nFiles: /opt/example/a\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseFileReferencesWrongPath, pass, fail).test_and_snapshot();
    }
}
