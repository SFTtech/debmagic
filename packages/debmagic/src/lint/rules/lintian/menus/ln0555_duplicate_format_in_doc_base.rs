use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DuplicateFormatInDocBase;

declare_rule! {
    /// A format is declared in more than one section.
    ///
    /// Compared after trimming and lower-casing. Extra is that format.
    /// Location is the field's last line.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DuplicateFormatInDocBase,
    code = "LN0555",
    tag = "duplicate-format-in-doc-base",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DuplicateFormatInDocBase {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::DuplicateFormat, false);
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
            doc("Document: example\n\nFormat: text\n\nFormat: pdf\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\n\nFormat: text\n\nFormat: TEXT\n"),
            doc(
                "Document: example\n\nFormat: html\nIndex: /usr/share/doc/a\n\nFormat: html\nIndex: /usr/share/doc/b\n",
            ),
            doc("Document: example\n\nFormat: text\n\nFormat: TEXT\n").udeb(),
        ];
        BinaryPackageTester::new(DuplicateFormatInDocBase, pass, fail).test_and_snapshot();
    }
}
