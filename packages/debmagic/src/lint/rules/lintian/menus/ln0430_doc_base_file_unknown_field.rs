use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DocBaseFileUnknownField;

declare_rule! {
    /// A doc-base field is not known in the section it appears in.
    ///
    /// Direct children of `usr/share/doc-base` are parsed. The first section
    /// allows `Document`, `Title`, `Section`, `Abstract`, and `Author`. After
    /// a blank line, later sections allow `Format`, `Files`, and `Index`.
    /// Extra is the field name. Location is the field's last line. Bytes that
    /// are not UTF-8 are skipped. Udebs are checked too.
    DocBaseFileUnknownField,
    code = "LN0430",
    tag = "doc-base-file-unknown-field",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseFileUnknownField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let findings = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            scan::doc_base_files(files)
        };
        for file in findings {
            for (line, field) in file.unknown_fields {
                ctx.diagnostic(Diagnostic::error(field).with_location(Location {
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

    fn doc(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/doc-base/example", TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            doc("Document: example\nTitle: T\nSection: S\nAuthor: A\nAbstract: line\n"),
            doc("Document: example\nTitle: T\n\nFormat: text\nFiles: /usr/share/doc/example/a\n"),
            doc("Document: example\nTitle: T\n\nFormat: text\n Index: /usr/share/doc/example/a\n"),
        ];
        let fail = vec![
            doc("Document: example\nTitle: T\nUnknown: Some field\n"),
            doc("Document: example\nTitle: T\n\nFormat: text\nUnknown: Some field\n"),
            doc("Document: example\nTitle: T\nFoo: bar\n more\n"),
            doc("Document: example\nUnknown: x\n").udeb(),
        ];
        BinaryPackageTester::new(DocBaseFileUnknownField, pass, fail).test_and_snapshot();
    }
}
