use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::files;

pub struct NotUsingPoDebconf;

declare_rule! {
    /// The package has a debconf template with a translatable description,
    /// but neither `debian/po` nor `debian/po.in` is a directory.
    ///
    /// Shared templates (`shared/packages-wordlist`, `shared/packages-ispell`),
    /// `/languages` templates and `for internal use` descriptions do not count.
    /// Extra is empty and there is no Location (the Subject is the package).
    NotUsingPoDebconf,
    code = "LN0908",
    tag = "not-using-po-debconf",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NotUsingPoDebconf {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(tree) = ctx.files() else {
            return;
        };
        if files::has_translatable_template(tree) && files::po_directory(tree).is_none() {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str =
        "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n";

    fn tree(templates: &str) -> Vec<(String, TestFile)> {
        vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            ("debian/templates".to_string(), TestFile::new(templates)),
        ]
    }

    fn with_po(templates: &str) -> Vec<(String, TestFile)> {
        let mut files = tree(templates);
        files.push(("debian/po".to_string(), TestFile::directory()));
        files
    }

    const ASKS: &str = "Template: example/foo\nType: string\nDescription: Ask\n Question.\n";

    #[test]
    fn test() {
        let pass = vec![
            // No translatable description.
            tree("Template: shared/packages-wordlist\nDescription: words\n"),
            tree("Template: example/foo\nType: string\nDescription: for internal use\n"),
            tree("Template: example/languages\nDescription: Languages\n"),
            // A blank line ends the template before its description.
            tree("Template: example/foo\n\nDescription: Ask\n Question.\n"),
            // A PO directory is present.
            with_po(ASKS),
            vec![
                ("debian/control".to_string(), TestFile::from(CONTROL)),
                ("debian/templates".to_string(), TestFile::new(ASKS)),
                ("debian/po.in".to_string(), TestFile::directory()),
            ],
        ];
        let fail = vec![
            tree(ASKS),
            tree("# translators\nTemplate: example/foo\nDescription: Ask\n"),
        ];
        SourceTreeTester::new(NotUsingPoDebconf, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(with_po(ASKS))];
        let fail = vec![SourcePackageCase::from(tree(ASKS))];
        SourcePackageTester::new(NotUsingPoDebconf, pass, fail).test_and_snapshot();
    }
}
