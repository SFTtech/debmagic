use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::files;

pub struct MisnamedPoFile;

declare_rule! {
    /// A file in `debian/po` (or `debian/po.in`) is not named for a language.
    ///
    /// `.po` files and directories are checked. A translation name is two or
    /// three lowercase letters, an optional `_XX` region, and an optional
    /// `@modifier`, then `.po`. Extra is empty. The Location is the file.
    MisnamedPoFile,
    code = "LN0381",
    tag = "misnamed-po-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MisnamedPoFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(tree) = ctx.files() else {
            return;
        };
        let Some(directory) = files::po_directory(tree) else {
            return;
        };
        let paths: Vec<_> = files::po_children(tree, directory)
            .into_iter()
            .filter(|entry| files::basename(entry).is_none_or(|name| !files::is_po_name(name)))
            .map(files::path_buf)
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const PO: &str = "msgid \"a\"\nmsgstr \"b\"\n";

    fn tree(paths: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![
            (
                "debian/control".to_string(),
                TestFile::from(
                    "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
                ),
            ),
            ("debian/po".to_string(), TestFile::directory()),
        ];
        files.extend(
            paths
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone())),
        );
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[("debian/po/de.po", TestFile::new(PO))]),
            tree(&[("debian/po/pt_BR.po", TestFile::new(PO))]),
            tree(&[("debian/po/templates.pot", TestFile::new(PO))]),
        ];
        let fail = vec![
            tree(&[("debian/po/English.po", TestFile::new(PO))]),
            tree(&[
                ("debian/po/lang.po", TestFile::new(PO)),
                ("debian/po/sample-file.po", TestFile::new(PO)),
            ]),
            tree(&[("debian/po/extra", TestFile::directory())]),
        ];
        SourceTreeTester::new(MisnamedPoFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(&[(
            "debian/po/de.po",
            TestFile::new(PO),
        )]))];
        let fail = vec![SourcePackageCase::from(tree(&[(
            "debian/po/English.po",
            TestFile::new(PO),
        )]))];
        SourcePackageTester::new(MisnamedPoFile, pass, fail).test_and_snapshot();
    }
}
