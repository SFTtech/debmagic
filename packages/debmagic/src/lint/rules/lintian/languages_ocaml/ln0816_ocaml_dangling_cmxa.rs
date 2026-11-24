use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::sibling::sibling;

pub struct OcamlDanglingCmxa;

declare_rule! {
    /// A `*.cmxa` byte code library is installed without a `*.a` static
    /// library of the same name in the same directory.
    ///
    /// Every Data files entry whose name ends in `.cmxa` is checked for a
    /// sibling with the same stem and extension `.a`. Any kind of entry counts
    /// as the sibling, as lintian's lookup does. Reported at the `.cmxa` path
    /// with an empty extra. Udebs are checked too.
    OcamlDanglingCmxa,
    code = "LN0816",
    tag = "ocaml-dangling-cmxa",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for OcamlDanglingCmxa {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .map(|entry| entry.path())
            .filter(|path| path.to_str().is_some_and(|name| name.ends_with(".cmxa")))
            .filter(|path| sibling(path, "a").is_some_and(|archive| files.get(&archive).is_none()))
            .map(Path::to_path_buf)
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
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

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            tree(vec![
                ("usr/lib/ocaml/foo.cmxa", TestFile::new("x")),
                ("usr/lib/ocaml/foo.a", TestFile::new("x")),
            ]),
            // Any kind of entry is the sibling.
            tree(vec![
                ("usr/lib/ocaml/foo.cmxa", TestFile::new("x")),
                ("usr/lib/ocaml/foo.a", TestFile::directory()),
            ]),
            // Only the `.cmxa` extension is checked.
            tree(vec![("usr/lib/ocaml/foo.cmxa.txt", TestFile::new("x"))]),
            tree(vec![("usr/lib/ocaml/foo.cmx", TestFile::new("x"))]),
            tree(vec![
                ("usr/lib/ocaml/a.b.cmxa", TestFile::new("x")),
                ("usr/lib/ocaml/a.b.a", TestFile::new("x")),
            ]),
        ];
        let fail = vec![
            tree(vec![("usr/lib/ocaml/foo.cmxa", TestFile::new("x"))]),
            // The sibling must share the directory.
            tree(vec![
                ("usr/lib/ocaml/foo.cmxa", TestFile::new("x")),
                ("usr/lib/foo.a", TestFile::new("x")),
            ]),
            tree(vec![
                ("usr/lib/ocaml/foo.cmxa", TestFile::new("x")),
                ("usr/lib/ocaml/bar.a", TestFile::new("x")),
            ]),
            tree(vec![(
                "usr/lib/ocaml/foo.cmxa",
                TestFile::symlink("bar.cmxa"),
            )]),
            tree(vec![("usr/lib/ocaml/a.b.cmxa", TestFile::new("x"))]),
            tree(vec![("usr/lib/ocaml/foo.cmxa", TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(OcamlDanglingCmxa, pass, fail).test_and_snapshot();
    }
}
