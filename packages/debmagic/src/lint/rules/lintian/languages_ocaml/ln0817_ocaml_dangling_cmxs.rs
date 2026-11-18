use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::sibling::sibling;

pub struct OcamlDanglingCmxs;

declare_rule! {
    /// A `*.cmxs` native plugin in a `lib*` package has no byte code sibling.
    ///
    /// Every Data files entry whose name ends in `.cmxs` is checked, in
    /// packages whose name starts with `lib`, for a sibling with the same stem
    /// and extension `.cma` or `.cmo`. Any kind of entry counts as the sibling.
    /// Reported at the `.cmxs` path with an empty extra. Udebs are checked too.
    OcamlDanglingCmxs,
    code = "LN0817",
    tag = "ocaml-dangling-cmxs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for OcamlDanglingCmxs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !ctx.package_name().starts_with("lib") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .map(|entry| entry.path())
            .filter(|path| path.to_str().is_some_and(|name| name.ends_with(".cmxs")))
            .filter(|path| {
                !["cma", "cmo"].iter().any(|extension| {
                    sibling(path, extension).is_some_and(|other| files.get(&other).is_some())
                })
            })
            .map(Path::to_path_buf)
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
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
        .package("libfoo")
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            tree(vec![
                ("usr/lib/ocaml/bar.cmxs", TestFile::new("x")),
                ("usr/lib/ocaml/bar.cma", TestFile::new("x")),
            ]),
            tree(vec![
                ("usr/lib/ocaml/bar.cmxs", TestFile::new("x")),
                ("usr/lib/ocaml/bar.cmo", TestFile::new("x")),
            ]),
            tree(vec![
                ("usr/lib/ocaml/a.b.cmxs", TestFile::new("x")),
                ("usr/lib/ocaml/a.b.cma", TestFile::new("x")),
            ]),
            // Only packages named `lib*` are checked.
            BinaryPackageCase::from(vec![(
                "usr/lib/ocaml/bar.cmxs".to_string(),
                TestFile::new("x"),
            )])
            .package("foo"),
            // Only the `.cmxs` extension is checked.
            tree(vec![("usr/lib/ocaml/bar.cmxs.txt", TestFile::new("x"))]),
            tree(vec![("usr/lib/ocaml/bar.cmx", TestFile::new("x"))]),
        ];
        let fail = vec![
            tree(vec![("usr/lib/ocaml/bar.cmxs", TestFile::new("x"))]),
            // The sibling must share the directory.
            tree(vec![
                ("usr/lib/ocaml/bar.cmxs", TestFile::new("x")),
                ("usr/lib/bar.cma", TestFile::new("x")),
            ]),
            // The extension must be `.cma` or `.cmo`.
            tree(vec![
                ("usr/lib/ocaml/bar.cmxs", TestFile::new("x")),
                ("usr/lib/ocaml/bar.cmx", TestFile::new("x")),
            ]),
            tree(vec![("usr/lib/ocaml/a.b.cmxs", TestFile::new("x"))]),
            tree(vec![("usr/lib/ocaml/bar.cmxs", TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(OcamlDanglingCmxs, pass, fail).test_and_snapshot();
    }
}
