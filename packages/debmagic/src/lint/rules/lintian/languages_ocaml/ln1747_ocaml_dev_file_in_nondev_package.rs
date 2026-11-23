use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::dev_files::{is_dev_file, is_dev_package, summary};

pub struct OcamlDevFileInNondevPackage;

declare_rule! {
    /// A package that is not an OCaml development package ships `*.cmi`,
    /// `*.cmx` or `*.cmxa` files.
    ///
    /// Those files are for compiling against the library, so they belong in a
    /// development package. The package is a development package when its
    /// name ends in `-dev`, or is `camlp4`, `camlp4-extra`, `camlp5`,
    /// `camlp5-extra`, `ocaml`, `ocaml-nox`, `ocaml-interp` or
    /// `ocaml-compiler-libs`. Every installed entry whose name ends in one of
    /// those extensions counts, directories included. `*.cma`, `*.cmo` and
    /// `*.cmxs` do not count, as they can be plugins. Reported as a pedantic
    /// hint with Extra `<N> file(s) in <directory>`, the longest directory that
    /// holds all of them, and no Location. Udebs are checked too.
    OcamlDevFileInNondevPackage,
    code = "LN1747",
    tag = "ocaml-dev-file-in-nondev-package",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for OcamlDevFileInNondevPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if is_dev_package(ctx.package_name()) {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let names: Vec<String> = files
            .entries()
            .iter()
            .filter_map(|entry| entry.path().to_str().map(str::to_string))
            .filter(|name| is_dev_file(name))
            .collect();
        if names.is_empty() {
            return;
        }
        let message = summary(names.iter().map(String::as_str));
        ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Pedantic));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(name: &str, files: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .iter()
                .map(|file| (*file, TestFile::new("")))
                .collect::<Vec<_>>(),
        )
        .package(name)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            case("libfoo-dev", &["usr/lib/ocaml/foo/foo.cmi"]),
            case("ocaml-nox", &["usr/lib/ocaml/stdlib/stdlib.cmi"]),
            // Plugins, not development files.
            case(
                "libfoo",
                &["usr/lib/ocaml/foo/foo.cma", "usr/lib/ocaml/foo/foo.cmxs"],
            ),
            case("libfoo", &["usr/lib/ocaml/foo/foo.cmo"]),
            case("libfoo", &["usr/lib/ocaml/foo/foo.ml"]),
        ];
        let fail = vec![
            case("libfoo", &["usr/lib/ocaml/foo/foo.cmi"]),
            case(
                "libfoo",
                &["usr/lib/ocaml/foo/foo.cmi", "usr/lib/ocaml/bar/bar.cmx"],
            ),
            case("libfoo", &["usr/lib/ocaml/foo/foo.cmxa"]),
            case("libfoo", &["usr/lib/ocaml/foo/foo.cmi"]).udeb(),
        ];
        BinaryPackageTester::new(OcamlDevFileInNondevPackage, pass, fail).test_and_snapshot();
    }
}
