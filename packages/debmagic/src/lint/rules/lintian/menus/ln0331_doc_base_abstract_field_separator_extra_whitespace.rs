use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DocBaseAbstractFieldSeparatorExtraWhitespace;

declare_rule! {
    /// An `Abstract` continuation is a paragraph separator with extra whitespace.
    ///
    /// A separator is a line of whitespace, a dot, and optional trailing
    /// whitespace. Exactly one space and nothing after the dot is correct.
    /// The first line of the field is not a continuation. The reported line
    /// follows lintian's `last_line - $#vals + idx`. Extra is empty. Location
    /// is the doc-base file.
    DocBaseAbstractFieldSeparatorExtraWhitespace,
    code = "LN0331",
    tag = "doc-base-abstract-field-separator-extra-whitespace",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseAbstractFieldSeparatorExtraWhitespace {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let findings = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            scan::doc_base_files(files)
        };
        for file in findings {
            for line in file.abstract_separators {
                ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                    path: file.path.clone(),
                    line: Some(line),
                    column: None,
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn doc(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            format!("usr/share/doc-base/{name}"),
            TestFile::new(body),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            doc("ok", "Abstract: title\n .\nSection: Help\n"),
            doc("first", "Abstract:  .\nSection: Help\n"),
            doc("title", "Title: foo\n  .\nSection: Help\n"),
            doc("gap", "Abstract: foo\n\n  .\nSection: Help\n"),
            doc(
                "binary",
                "Document: binary\nTitle: t\nAuthor: a\nAbstract: intro\n files\n fourth one has trailing whitespace.\n .\n This section has a speling error.\n .\n The above separator was fine.\nSection: X\n",
            ),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe, b'\n']),
            )]),
            doc("nested/file", "Abstract: foo\n  .\n"),
        ];
        let fail = vec![
            doc("two", "Abstract: title\n  .\nSection: Help\n"),
            doc("trail", "Abstract: title\n . \nSection: Help\n"),
            doc("tab", "Abstract: title\n\t.\nSection: Help\n"),
            doc("both", "Abstract: title\n  .\n more\n .\t\nSection: Help\n"),
            doc(
                "legacy",
                "Document: binary!docs  \nTitle: Broken\nAuthor: Russ Allbery\nAbstract: This control file exercises various tests of doc-base control\n  files, including several things that aren't tested yet.  The third and\n  fourth one has trailing whitespace.\n  .\n This section has a speling error.\n .\n The above separator was fine.\nSection: Non/Existant\n",
            ),
        ];
        BinaryPackageTester::new(DocBaseAbstractFieldSeparatorExtraWhitespace, pass, fail)
            .test_and_snapshot();
    }
}
