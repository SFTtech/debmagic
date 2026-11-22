use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::shared::{is_empty, objects, strong};

pub struct UndeclaredElfPrerequisites;

declare_rule! {
    /// An ELF object has `NEEDED` entries, but the package declares no strong
    /// relation (`Pre-Depends` or `Depends`).
    ///
    /// Reported at each ELF executable or shared object with at least one
    /// `NEEDED` entry, when the Binary package's `Pre-Depends` and `Depends` are
    /// both absent or empty. The extra is the unique `NEEDED` names, sorted and
    /// wrapped in parentheses, for example `(libc.so.6 libm.so.6)`. Skipped for
    /// udebs. The Location is the file.
    UndeclaredElfPrerequisites,
    code = "LN1299",
    tag = "undeclared-elf-prerequisites",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UndeclaredElfPrerequisites {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        if !is_empty(&strong(ctx)) {
            return;
        }
        let reports: Vec<(std::path::PathBuf, String)> = objects(ctx)
            .into_iter()
            .filter(|object| !object.needed.is_empty())
            .map(|object| {
                let mut needed = object.needed;
                needed.sort();
                needed.dedup();
                (object.path, format!("({})", needed.join(" ")))
            })
            .collect();
        for (path, extra) in reports {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
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
            case(Some("libc6"), vec![("usr/bin/foo", elf(&["libc.so.6"]))]),
            case(Some("libfoo"), vec![("usr/bin/foo", elf(&["libc.so.6"]))]),
            case(None, vec![("usr/bin/foo", elf(&[]))]),
            // Not an ELF file.
            case(None, vec![("usr/bin/script", TestFile::new("#!/bin/sh\n"))]),
            // A relocatable object is not visited.
            case(
                None,
                vec![(
                    "usr/lib/foo.o",
                    TestFile::from_bytes(TestElf::relocatable().needed("libc.so.6").build()),
                )],
            ),
            // A udeb is not checked.
            case(None, vec![("usr/bin/foo", elf(&["libc.so.6"]))]).udeb(),
        ];
        let fail = vec![
            case(None, vec![("usr/bin/foo", elf(&["libc.so.6"]))]),
            // Sorted and unique in the extra; one hint per file.
            case(
                Some(""),
                vec![("usr/bin/foo", elf(&["libm.so.6", "libc.so.6", "libm.so.6"]))],
            ),
            case(
                None,
                vec![
                    ("usr/bin/b", elf(&["libc.so.6"])),
                    (
                        "usr/lib/a.so",
                        TestFile::from_bytes(TestElf::shared_object().needed("libm.so.6").build()),
                    ),
                ],
            ),
        ];
        BinaryPackageTester::new(UndeclaredElfPrerequisites, pass, fail).test_and_snapshot();
    }
}
