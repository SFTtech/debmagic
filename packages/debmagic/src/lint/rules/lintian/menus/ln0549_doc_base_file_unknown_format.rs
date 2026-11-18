use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseFileUnknownFormat;

declare_rule! {
    /// A `Format` value is not one of the known formats.
    ///
    /// Known are `html`, `text`, `pdf`, `postscript`, `info`, `dvi`, and
    /// `debiandoc-sgml`; the value is trimmed and lower-cased first. Extra is
    /// that value. Location is the field's last line.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseFileUnknownFormat,
    code = "LN0549",
    tag = "doc-base-file-unknown-format",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseFileUnknownFormat {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::UnknownFormat, true);
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
            doc("Document: example\n\nFormat: HTML\nIndex: /usr/share/doc/example/index.html\n"),
            doc("Document: example\n\nFormat: DebianDoc-SGML\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\n\nFormat: ESP\n"),
            doc("Document: example\n\nFormat: text \n markdown\n"),
            doc("Document: example\n\nFormat: ESP\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseFileUnknownFormat, pass, fail).test_and_snapshot();
    }
}
