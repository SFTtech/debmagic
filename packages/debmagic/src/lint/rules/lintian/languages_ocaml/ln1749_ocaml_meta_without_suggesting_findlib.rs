use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct OcamlMetaWithoutSuggestingFindlib;

declare_rule! {
    /// A package installs an OCaml `META` file but neither depends on, suggests,
    /// nor recommends `ocaml-findlib`.
    ///
    /// A library with a `META` file is easier to use with `findlib`, so the
    /// package should at least suggest `ocaml-findlib`. The Rule looks for an
    /// installed entry under `usr/lib/ocaml/` whose path has `/META` followed
    /// by a `.` or the end of the name, and then asks whether the package's
    /// `Pre-Depends`, `Depends`, `Recommends` and `Suggests` satisfy
    /// `ocaml-findlib`. Reported as a pedantic hint with no Extra and no
    /// Location. Udebs are checked too.
    OcamlMetaWithoutSuggestingFindlib,
    code = "LN1749",
    tag = "ocaml-meta-without-suggesting-findlib",
    default_selected = false,
    experimental = false,
}

static META: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/META(?:\.|$)").expect("valid regex"));

impl BinaryPackageRule for OcamlMetaWithoutSuggestingFindlib {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let has_meta = files.entries().iter().any(|entry| {
            entry
                .path()
                .to_str()
                .is_some_and(|name| name.starts_with("usr/lib/ocaml/") && META.is_match(name))
        });
        if !has_meta {
            return;
        }
        let relations = ctx.installation_control().fields().relations_and(&[
            "Pre-Depends",
            "Depends",
            "Recommends",
            "Suggests",
        ]);
        if !relations.satisfies("ocaml-findlib") {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Pedantic));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(file: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(file, TestFile::new("name: foo\n"))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Not under usr/lib/ocaml/.
            case("usr/lib/foo/META"),
            case("usr/share/ocaml/foo/META"),
            // Not a META file.
            case("usr/lib/ocaml/foo/METAL"),
            // Suggests, recommends or depends on ocaml-findlib.
            case("usr/lib/ocaml/foo/META").field("Suggests", "ocaml-findlib"),
            case("usr/lib/ocaml/foo/META").field("Recommends", "ocaml-findlib"),
            case("usr/lib/ocaml/foo/META").field("Depends", "ocaml-findlib (>= 1.9)"),
        ];
        let fail = vec![
            case("usr/lib/ocaml/foo/META"),
            case("usr/lib/ocaml/foo/META.foo"),
            case("usr/lib/ocaml/foo/META").field("Depends", "ocaml-nox"),
            case("usr/lib/ocaml/foo/META").udeb(),
        ];
        BinaryPackageTester::new(OcamlMetaWithoutSuggestingFindlib, pass, fail).test_and_snapshot();
    }
}
