use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseUnknownSection;

declare_rule! {
    /// The `Section` value is not a known doc-base section.
    ///
    /// A new subsection of a known section is fine, and `Apps/` or
    /// `Applications/` prefixes have their own Tag. Extra is the value.
    /// Location is the field's last line.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseUnknownSection,
    code = "LN0552",
    tag = "doc-base-unknown-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseUnknownSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::UnknownSection, true);
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
            doc("Document: example\nSection: Help\n"),
            doc("Document: example\nSection: Help/Extra\n"),
            doc("Document: example\nSection: Apps/Programming\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\nSection: Non/Existant\n"),
            doc("Document: example\nSection: Apps/Nonexistent\n"),
            doc("Document: example\nSection: Non/Existant\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseUnknownSection, pass, fail).test_and_snapshot();
    }
}
