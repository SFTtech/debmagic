use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::doc_base::{self, Kind};

pub struct SpellingErrorInDocBaseTitleField;

declare_rule! {
    /// The `Title` field of a doc-base section has a misspelling, a repeated
    /// word, or a capitalization error from lintian's lists, such as `debian`
    /// for `Debian`.
    ///
    /// The field's values, joined by spaces, are checked as lintian's
    /// `check_spelling` and `check_spelling_picky` check them, with the
    /// spelling exceptions of the Binary package's group. Each finding is
    /// reported with the field's line and extra `<found> <correction>`. Pedantic
    /// and not selected by default, as in lintian.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are UTF-8.
    /// Udebs are checked too.
    SpellingErrorInDocBaseTitleField,
    code = "LN1468",
    tag = "spelling-error-in-doc-base-title-field",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for SpellingErrorInDocBaseTitleField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        doc_base::report_spelling(ctx, Kind::SpellingTitle);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn doc(title: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/doc-base/example",
            TestFile::new(format!(
                "Document: example\nTitle: {title}\nSection: Help\nAbstract: A description.\n\nFormat: text\nFiles: /usr/share/doc/example/a\n"
            )),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            doc("Example Manual"),
        ];
        let fail = vec![
            doc("debian Example"),
            doc("A speling manual").udeb(),
            doc("the the manual"),
        ];
        BinaryPackageTester::new(SpellingErrorInDocBaseTitleField, pass, fail).test_and_snapshot();
    }
}
