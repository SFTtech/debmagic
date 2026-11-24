use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct SpellingErrorInDocBaseAbstractField;

declare_rule! {
    /// The `Abstract` field of a doc-base section has a misspelling, a repeated
    /// word or a misspelled phrase from lintian's spelling lists, or a
    /// capitalization error of the picky list.
    ///
    /// The field's values, joined by spaces, are checked as lintian's
    /// `check_spelling` and `check_spelling_picky` check them, with the
    /// spelling exceptions of the Binary package's group. Each finding is
    /// reported with the field's line and extra `<found> <correction>`. Pedantic
    /// and not selected by default, as in lintian.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are UTF-8.
    /// Udebs are checked too.
    SpellingErrorInDocBaseAbstractField,
    code = "LN1423",
    tag = "spelling-error-in-doc-base-abstract-field",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for SpellingErrorInDocBaseAbstractField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report_spelling(ctx, Kind::SpellingAbstract);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn doc(abstract_text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/doc-base/example",
            TestFile::new(format!(
                "Document: example\nTitle: Example\nSection: Help\nAbstract: {abstract_text}\n\nFormat: text\nFiles: /usr/share/doc/example/a\n"
            )),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            doc("A correct description of the package."),
            // A word of the acronym list is not a misspelling.
            doc("The USA tool."),
        ];
        let fail = vec![
            doc("This has a speling error."),
            doc("A repeated repeated word."),
            doc("debian package.").udeb(),
        ];
        BinaryPackageTester::new(SpellingErrorInDocBaseAbstractField, pass, fail)
            .test_and_snapshot();
    }
}
