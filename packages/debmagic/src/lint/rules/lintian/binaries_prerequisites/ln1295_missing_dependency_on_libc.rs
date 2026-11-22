use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::shared::{is_empty, names_package_prefix, objects, sorted_unique, strong};

pub struct MissingDependencyOnLibc;

declare_rule! {
    /// An ELF object needs a `libc` version that the package does not depend on.
    ///
    /// Every `NEEDED` entry `libc.so.<N>` (with anything after `N`) maps to the
    /// package `libc<N>`. The Rule is skipped for udebs, for a Binary package
    /// with no `Pre-Depends` or `Depends`, and for the libc packages themselves
    /// (`libc<version>` with `-` or the end after the version). A package that
    /// the strong relations already name (a prefix match that ends at a word
    /// boundary, so `libc6-dev` names `libc6`) is not reported. Otherwise each
    /// missing package is reported with the extra `needed by <first file> and
    /// <N> others`, where the first file is the smallest path that needs it
    /// and the count is the other unique paths. The Rule has no Location.
    MissingDependencyOnLibc,
    code = "LN1295",
    tag = "missing-dependency-on-libc",
    default_selected = true,
    experimental = false,
}

static LIBC_SELF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^libc[\d.]+(?:-|$)").expect("valid regex"));

impl BinaryPackageRule for MissingDependencyOnLibc {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let depends = strong(ctx);
        if is_empty(&depends) {
            return;
        }
        let self_is_libc = LIBC_SELF.is_match(ctx.package_name());

        let mut needed_by: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for object in objects(ctx) {
            for library in &object.needed {
                let Some(version) = library.strip_prefix("libc.so.") else {
                    continue;
                };
                if !version.starts_with(|c: char| c.is_ascii_digit()) {
                    continue;
                }
                needed_by
                    .entry(format!("libc{version}"))
                    .or_default()
                    .push(object.path.clone());
            }
        }

        if self_is_libc {
            return;
        }
        for (package, paths) in needed_by {
            if names_package_prefix(&depends, &package) {
                continue;
            }
            let files = sorted_unique(&paths);
            let mut context = format!("needed by {}", files[0]);
            if files.len() > 1 {
                context.push_str(&format!(" and {} others", files.len() - 1));
            }
            ctx.diagnostic(Diagnostic::error(context));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(needed: &[&str]) -> TestFile {
        let mut elf = TestElf::executable();
        for library in needed {
            elf = elf.needed(library);
        }
        TestFile::from_bytes(elf.build())
    }

    fn case(
        package: &str,
        depends: Option<&str>,
        files: Vec<(&str, TestFile)>,
    ) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(files)
            .package(package)
            .field("Architecture", "amd64");
        if let Some(depends) = depends {
            case = case.field("Depends", depends);
        }
        case
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Nothing needed, or no strong relation to check.
            case("example", Some("libc6"), vec![("usr/bin/foo", elf(&[]))]),
            case("example", None, vec![("usr/bin/foo", elf(&["libc.so.6"]))]),
            // The package depends on libc6 (or a package it prefixes).
            case(
                "example",
                Some("libc6 (>= 2.34)"),
                vec![("usr/bin/foo", elf(&["libc.so.6"]))],
            ),
            case(
                "example",
                Some("libc6-dev"),
                vec![("usr/bin/foo", elf(&["libc.so.6"]))],
            ),
            // A libc package itself is not reported.
            case(
                "libc6",
                Some("libc-bin"),
                vec![("usr/bin/foo", elf(&["libc.so.6"]))],
            ),
            // `libc.so.6x` is package `libc6x`, which the relation names.
            case(
                "example",
                Some("libc6x"),
                vec![("usr/bin/foo", elf(&["libc.so.6x"]))],
            ),
            // Shared object only in a relocatable object is not visited.
            case(
                "example",
                Some("libfoo"),
                vec![(
                    "usr/lib/foo.o",
                    TestFile::from_bytes(TestElf::relocatable().needed("libc.so.6").build()),
                )],
            ),
            // Not an ELF file.
            case(
                "example",
                Some("libfoo"),
                vec![("usr/bin/script", TestFile::new("#!/bin/sh\n"))],
            ),
            // A udeb is not checked.
            case(
                "example",
                Some("libfoo"),
                vec![("usr/bin/foo", elf(&["libc.so.6"]))],
            )
            .udeb(),
        ];
        let fail = vec![
            case(
                "example",
                Some("libfoo"),
                vec![("usr/bin/foo", elf(&["libc.so.6"]))],
            ),
            case(
                "example",
                Some("libfoo"),
                vec![
                    ("usr/bin/b", elf(&["libc.so.6"])),
                    (
                        "usr/lib/a.so",
                        TestFile::from_bytes(TestElf::shared_object().needed("libc.so.6").build()),
                    ),
                    ("usr/bin/c", elf(&["libc.so.6", "libc.so.6"])),
                ],
            ),
            // A libc version other than 6 is a different package.
            case(
                "example",
                Some("libc6"),
                vec![("usr/bin/foo", elf(&["libc.so.7"]))],
            ),
        ];
        BinaryPackageTester::new(MissingDependencyOnLibc, pass, fail).test_and_snapshot();
    }
}
