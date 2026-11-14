use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct DocBaseUsesApplicationsSection;

declare_rule! {
    /// The `Section` uses an obsolete `Apps/` or `Applications/` prefix.
    ///
    /// The rest after the prefix has to be a known section. Extra is the
    /// whole value. Location is the field's last line.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are
    /// UTF-8. Udebs are checked too.
    DocBaseUsesApplicationsSection,
    code = "LN0553",
    tag = "doc-base-uses-applications-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseUsesApplicationsSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report(ctx, Kind::UsesApplicationsSection, true);
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
            doc("Document: example\nSection: Programming\n"),
            doc("Document: example\nSection: Apps/Nonexistent\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
        ];
        let fail = vec![
            doc("Document: example\nSection: Apps/Programming\n"),
            doc("Document: example\nSection: Applications/Editors\n"),
            doc("Document: example\nSection: Apps/Programming\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseUsesApplicationsSection, pass, fail).test_and_snapshot();
    }
}
