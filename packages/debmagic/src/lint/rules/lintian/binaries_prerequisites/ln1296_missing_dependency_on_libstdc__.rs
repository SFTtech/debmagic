use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::shared::{is_empty, names_package_prefix, objects, sorted_unique, strong};

pub struct MissingDependencyOnLibstdc;

declare_rule! {
    /// An ELF object needs a `libstdc++` version that the package does not depend on.
    ///
    /// Every `NEEDED` entry `libstdc++.so.<N>` (exactly that, with digits for
    /// `N`) maps to the package `libstdc++<N>`. Skipped for udebs and for a
    /// Binary package with no `Pre-Depends` or `Depends`. A package that the
    /// strong relations already name, at a word boundary, is not reported.
    /// Otherwise each missing package is reported with the extra `needed by
    /// <first file> and <N> others`, as in `missing-dependency-on-libc`. The
    /// Rule has no Location. Severity is error; the Tag is experimental and
    /// not selected by default, as in lintian.
    MissingDependencyOnLibstdc,
    code = "LN1296",
    tag = "missing-dependency-on-libstdc++",
    default_selected = false,
    experimental = true,
}

impl BinaryPackageRule for MissingDependencyOnLibstdc {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let depends = strong(ctx);
        if is_empty(&depends) {
            return;
        }

        let mut needed_by: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for object in objects(ctx) {
            for library in &object.needed {
                let Some(version) = library.strip_prefix("libstdc++.so.") else {
                    continue;
                };
                if version.is_empty() || !version.bytes().all(|b| b.is_ascii_digit()) {
                    continue;
                }
                needed_by
                    .entry(format!("libstdc++{version}"))
                    .or_default()
                    .push(object.path.clone());
            }
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

    fn case(depends: Option<&str>, files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(files).field("Architecture", "amd64");
        if let Some(depends) = depends {
            case = case.field("Depends", depends);
        }
        case
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(
                Some("libstdc++6"),
                vec![("usr/bin/foo", elf(&["libstdc++.so.6"]))],
            ),
            case(
                Some("libstdc++6 (>= 12)"),
                vec![("usr/bin/foo", elf(&["libstdc++.so.6"]))],
            ),
            case(None, vec![("usr/bin/foo", elf(&["libstdc++.so.6"]))]),
            case(Some("libfoo"), vec![("usr/bin/foo", elf(&["libc.so.6"]))]),
            // Not a libstdc++ soname with a version number.
            case(
                Some("libfoo"),
                vec![("usr/bin/foo", elf(&["libstdc++.so.6x"]))],
            ),
            case(
                Some("libfoo"),
                vec![("usr/bin/foo", elf(&["libstdc++.so."]))],
            ),
            case(
                Some("libfoo"),
                vec![("usr/bin/foo", elf(&["libstdc++.so.6"]))],
            )
            .udeb(),
        ];
        let fail = vec![
            case(
                Some("libfoo"),
                vec![("usr/bin/foo", elf(&["libstdc++.so.6"]))],
            ),
            case(
                Some("libfoo"),
                vec![
                    ("usr/bin/b", elf(&["libstdc++.so.6"])),
                    ("usr/bin/a", elf(&["libstdc++.so.6"])),
                ],
            ),
        ];
        BinaryPackageTester::new(MissingDependencyOnLibstdc, pass, fail).test_and_snapshot();
    }
}
