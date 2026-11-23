use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::dev_files::{is_dev_file, summary};

pub struct OcamlDevFileNotInUsrLibOcaml;

declare_rule! {
    /// An OCaml development file is installed outside
    /// `usr/lib/<multiarch>/ocaml/<abi>/`.
    ///
    /// `*.cmi`, `*.cmx` and `*.cmxa` files are used only for compilation, and
    /// belong in a subdirectory of the standard OCaml library path. Every
    /// installed entry whose name ends in one of those extensions counts,
    /// directories included, unless its path starts with
    /// `usr/lib/<dir>/ocaml/<dir>/`. `*.cma`, `*.cmo` and `*.cmxs` do not count,
    /// as they can be plugins. Reported as a pedantic hint with Extra
    /// `<N> file(s) in <directory>`, the longest directory that holds all of
    /// them, and no Location. Udebs are checked too.
    OcamlDevFileNotInUsrLibOcaml,
    code = "LN1748",
    tag = "ocaml-dev-file-not-in-usr-lib-ocaml",
    default_selected = false,
    experimental = false,
}

static STANDARD_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/[^/]+/ocaml/[^/]+/").expect("valid regex"));

impl BinaryPackageRule for OcamlDevFileNotInUsrLibOcaml {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let names: Vec<String> = files
            .entries()
            .iter()
            .filter_map(|entry| entry.path().to_str().map(str::to_string))
            .filter(|name| is_dev_file(name) && !STANDARD_PATH.is_match(name))
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
            case(&["usr/lib/x86_64-linux-gnu/ocaml/4.14/foo/foo.cmi"]),
            case(&["usr/lib/x86_64-linux-gnu/ocaml/4.14/foo/foo.cmxa"]),
            // Plugins, not development files.
            case(&[
                "usr/lib/foo/foo.cma",
                "usr/lib/foo/foo.cmxs",
                "usr/lib/foo/foo.cmo",
            ]),
            case(&["usr/share/foo/foo.ml"]),
        ];
        let fail = vec![
            case(&["usr/lib/foo.cmi"]),
            case(&["usr/lib/ocaml/foo/foo.cmi"]),
            case(&["usr/lib/x86_64-linux-gnu/ocaml/foo.cmi"]),
            case(&[
                "usr/lib/x86_64-linux-gnu/foo/foo.cmi",
                "usr/lib/foo/bar.cmx",
            ]),
            case(&["usr/lib/foo.cmi"]).udeb(),
        ];
        BinaryPackageTester::new(OcamlDevFileNotInUsrLibOcaml, pass, fail).test_and_snapshot();
    }
}
