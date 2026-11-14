use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseAbstractFieldIsTemplate;

declare_rule! {
    /// An `Abstract` continuation line is the doc-base template text.
    ///
    /// The first line of the field is skipped. A later line that matches
    /// `manage online manuals ... Debian` is the template, except in the
    /// `doc-base` package itself. Location is the field's last line. Extra is
    /// empty.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseAbstractFieldIsTemplate,
    code = "LN0540",
    tag = "doc-base-abstract-field-is-template",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseAbstractFieldIsTemplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::AbstractIsTemplate, true);
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
            doc("Document: example\nAbstract: manage online manuals for Debian\n"),
            doc("Document: example\nAbstract: x\n manage online manuals for Debian\n")
                .package("doc-base"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\nAbstract: x\n manage online manuals for Debian\n more\n"),
            doc("Document: example\nAbstract: x\n manage online manuals for Debian\n more\n")
                .udeb(),
        ];
        BinaryPackageTester::new(DocBaseAbstractFieldIsTemplate, pass, fail).test_and_snapshot();
    }
}
