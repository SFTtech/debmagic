use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::files;

pub struct MissingPotfilesIn;

declare_rule! {
    /// The PO directory has no `POTFILES.in`, or it is not a regular file.
    ///
    /// Only checked when the package uses po-debconf (see LN0908). A symbolic
    /// link to a regular file inside the tree counts as present. Extra is
    /// empty and there is no Location.
    MissingPotfilesIn,
    code = "LN0905",
    tag = "missing-potfiles-in",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MissingPotfilesIn {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(tree) = ctx.files() else {
            return;
        };
        if !files::has_translatable_template(tree) {
            return;
        }
        let Some(directory) = files::po_directory(tree) else {
            return;
        };
        if files::open_file(tree, &Path::new(directory).join("POTFILES.in")).is_none() {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str =
        "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n";
    const ASKS: &str = "Template: example/foo\nType: string\nDescription: Ask\n Question.\n";

    fn tree(extra: Vec<(&str, TestFile)>) -> Vec<(String, TestFile)> {
        let mut files = vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            ("debian/templates".to_string(), TestFile::new(ASKS)),
            ("debian/po".to_string(), TestFile::directory()),
        ];
        files.extend(
            extra
                .into_iter()
                .map(|(path, file)| (path.to_string(), file)),
        );
        files
    }

    #[test]
    fn test() {
        let pass = vec![tree(vec![("debian/po/POTFILES.in", TestFile::new(""))])];
        let fail = vec![
            tree(vec![]),
            tree(vec![("debian/po/POTFILES.in", TestFile::directory())]),
        ];
        SourceTreeTester::new(MissingPotfilesIn, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(vec![(
            "debian/po/POTFILES.in",
            TestFile::new(""),
        )]))];
        let fail = vec![SourcePackageCase::from(tree(vec![]))];
        SourcePackageTester::new(MissingPotfilesIn, pass, fail).test_and_snapshot();
    }
}
