use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::sibling::sibling;

pub struct OcamlStrayCmo;

declare_rule! {
    /// A `*.cma` byte code library is installed next to a `*.cmo` byte code
    /// file of the same base name.
    ///
    /// The module of the `*.cmo` file is usually an archive member of the
    /// `*.cma` library, so the `*.cmo` file is not needed. The sibling is the
    /// file in the same directory named with the `.cma` file's name minus its
    /// last extension, plus `.cmo`; any kind of entry counts. Reported as an
    /// info hint at the `*.cma` path, with no Extra. Udebs are checked too.
    OcamlStrayCmo,
    code = "LN1750",
    tag = "ocaml-stray-cmo",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for OcamlStrayCmo {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<std::path::PathBuf> = files
            .entries()
            .iter()
            .map(|entry| entry.path().to_path_buf())
            .filter(|path| path.to_str().is_some_and(|name| name.ends_with(".cma")))
            .filter(|path| sibling(path, "cmo").is_some_and(|cmo| files.get(&cmo).is_some()))
            .collect();
        for path in paths {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(files: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .iter()
                .map(|file| (*file, TestFile::new("")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            case(&["usr/lib/ocaml/foo/foo.cma"]),
            case(&["usr/lib/ocaml/foo/foo.cmo"]),
            // The .cmo file is in another directory.
            case(&["usr/lib/ocaml/foo/foo.cma", "usr/lib/ocaml/bar/foo.cmo"]),
            // The .cmo file has another base name.
            case(&["usr/lib/ocaml/foo/foo.cma", "usr/lib/ocaml/foo/bar.cmo"]),
        ];
        let fail = vec![
            case(&["usr/lib/ocaml/bleh.cma", "usr/lib/ocaml/bleh.cmo"]),
            case(&["usr/lib/ocaml/foo/foo.cma", "usr/lib/ocaml/foo/foo.cmo"]),
            case(&["usr/lib/ocaml/bleh.cma", "usr/lib/ocaml/bleh.cmo"]).udeb(),
        ];
        BinaryPackageTester::new(OcamlStrayCmo, pass, fail).test_and_snapshot();
    }
}
