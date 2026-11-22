use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::shared::objects;

pub struct ProgramNotLinkedAgainstLibc;

declare_rule! {
    /// A program (not a shared object) with `NEEDED` entries is not linked
    /// against libc.
    ///
    /// Reported at each ELF executable (`ET_EXEC` or PIE) that has at least one
    /// `NEEDED` entry, none of which is `libc.so.<...>` or starts with
    /// `libstdc++.so.`, since C++ programs link libstdc++ and not libc. Skipped
    /// for udebs and for shared objects. The Location is the file.
    ///
    /// Difference from lintian: lintian also skips Binary packages built with
    /// octave, which it recognises by the source's `Build-Depends` naming
    /// `dh-octave`. A Binary package has no access to its source's control
    /// data, so only the `octave-` package name prefix is skipped here.
    ProgramNotLinkedAgainstLibc,
    code = "LN1297",
    tag = "program-not-linked-against-libc",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ProgramNotLinkedAgainstLibc {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() || ctx.package_name().starts_with("octave-") {
            return;
        }
        let paths: Vec<PathBuf> = objects(ctx)
            .into_iter()
            .filter(|object| !object.shared && !object.needed.is_empty())
            .filter(|object| {
                !object.needed.iter().any(|library| {
                    library.starts_with("libc.so.") || library.starts_with("libstdc++.so.")
                })
            })
            .map(|object| object.path)
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

    fn case(package: &str, files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
            .package(package)
            .field("Architecture", "amd64")
            .field("Depends", "libfoo")
    }

    #[test]
    fn binary() {
        let pass = vec![
            case("example", vec![("usr/bin/foo", elf(&["libc.so.6"]))]),
            case(
                "example",
                vec![("usr/bin/foo", elf(&["libstdc++.so.6", "libm.so.6"]))],
            ),
            // No NEEDED entries: a static program.
            case("example", vec![("usr/bin/foo", elf(&[]))]),
            // A shared object is not a program.
            case(
                "example",
                vec![(
                    "usr/lib/libfoo.so.1",
                    TestFile::from_bytes(TestElf::shared_object().needed("libm.so.6").build()),
                )],
            ),
            // Packages built with octave are skipped by name.
            case("octave-foo", vec![("usr/bin/foo", elf(&["libm.so.6"]))]),
            // A udeb is not checked.
            case("example", vec![("usr/bin/foo", elf(&["libm.so.6"]))]).udeb(),
        ];
        let fail = vec![
            case("example", vec![("usr/bin/foo", elf(&["libm.so.6"]))]),
            case(
                "example",
                vec![(
                    "usr/bin/pie",
                    TestFile::from_bytes(TestElf::pie_executable().needed("libm.so.6").build()),
                )],
            ),
        ];
        BinaryPackageTester::new(ProgramNotLinkedAgainstLibc, pass, fail).test_and_snapshot();
    }
}
