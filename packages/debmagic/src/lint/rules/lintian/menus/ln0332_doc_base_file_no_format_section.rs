use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DocBaseFileNoFormatSection;

declare_rule! {
    /// A doc-base file never declares a `Format` field.
    ///
    /// Direct children of `usr/share/doc-base` are parsed. A `Format` field in
    /// any section is enough. Bytes that are not UTF-8 are skipped. Extra is
    /// empty. Location is the file, with no line.
    DocBaseFileNoFormatSection,
    code = "LN0332",
    tag = "doc-base-file-no-format-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocBaseFileNoFormatSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let findings = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            scan::doc_base_files(files)
        };
        for file in findings {
            if !file.no_format {
                continue;
            }
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: file.path,
                line: None,
                column: None,
            }));
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
            doc(
                "ok",
                "Document: ok\nTitle: T\nSection: Help\n\nFormat: text\nFiles: /usr/share/doc/ok/a.txt\n",
            ),
            doc(
                "later",
                "Document: ok\n\nFormat: HTML\nIndex: /usr/share/doc/ok/index.html\n",
            ),
            doc("empty-value", "Format:\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/bad",
                TestFile::from_bytes(vec![0xff, 0xfe]),
            )]),
            doc("dir/nested", ""),
            BinaryPackageCase::from(vec![("usr/share/doc/example/README", TestFile::new("x\n"))]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/link",
                TestFile::symlink("missing"),
            )]),
        ];
        let fail = vec![
            doc("space ", ""),
            doc("main", "Document: example\nTitle: T\nSection: Help\n"),
            BinaryPackageCase::from(vec![
                ("usr/share/doc-base/a", TestFile::new("")),
                (
                    "usr/share/doc-base/b",
                    TestFile::new("Format: text\nFiles: /usr/share/doc/b/a.txt\n"),
                ),
            ]),
        ];
        BinaryPackageTester::new(DocBaseFileNoFormatSection, pass, fail).test_and_snapshot();
    }
}
