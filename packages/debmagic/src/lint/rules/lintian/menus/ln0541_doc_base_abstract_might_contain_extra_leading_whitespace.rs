use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseAbstractMightContainExtraLeadingWhitespace;

declare_rule! {
    /// An `Abstract` is indented by more than one space.
    ///
    /// The first continuation line sets the indent; it is fine when it is one
    /// space. A wider indent stays suspicious until a later line is indented
    /// differently or starts with another character. Separator lines and the
    /// template are skipped. Location is the field's last line. Extra is empty.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseAbstractMightContainExtraLeadingWhitespace,
    code = "LN0541",
    tag = "doc-base-abstract-might-contain-extra-leading-whitespace",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseAbstractMightContainExtraLeadingWhitespace {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::AbstractExtraLeadingWhitespace, true);
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
            doc("Document: example\nAbstract: first\n second\n third\n"),
            doc("Document: example\nAbstract: first\n  * x\n   * y\n"),
            doc("Document: example\nAbstract: only one line\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\nAbstract: first\n  second\n  third\n"),
            doc("Document: example\nAbstract: first\n  second\n"),
            doc("Document: example\nAbstract: first\n  second\n  third\n").udeb(),
        ];
        BinaryPackageTester::new(
            DocBaseAbstractMightContainExtraLeadingWhitespace,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
