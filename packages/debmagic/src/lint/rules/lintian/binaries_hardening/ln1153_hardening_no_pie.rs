use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::hardening::lacking_hardening;
use crate::lint::rule::BinaryPackageRule;

pub struct HardeningNoPie;

declare_rule! {
    /// A dynamically linked executable is not position independent.
    ///
    /// Same selection as `hardening-no-relro`, with `pie` as the recommended
    /// feature, and the file is an `ET_EXEC` executable (`readelf` type
    /// `EXEC`). A PIE is `ET_DYN`; shared objects are never hits. Extra is
    /// empty; the Data files path is the Location.
    HardeningNoPie,
    code = "LN1153",
    tag = "hardening-no-pie",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for HardeningNoPie {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(paths) =
            lacking_hardening(ctx, "pie", |facts| facts.is_fixed_address_executable())
        else {
            return;
        };
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
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn amd64(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)]).field("Architecture", "amd64")
    }

    #[test]
    fn binary() {
        let pass = vec![
            amd64(
                "usr/bin/good",
                elf(TestElf::pie_executable().needed("libc.so.6")),
            ),
            amd64(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object()
                    .soname("libfoo.so.1")
                    .needed("libc.so.6")),
            ),
            // A static executable has nothing to harden.
            amd64("usr/bin/static", elf(TestElf::executable())),
            // Not an executable or shared object.
            amd64("usr/lib/foo.o", elf(TestElf::relocatable())),
            amd64("usr/bin/script", TestFile::new("#!/bin/sh\n")),
            // dpkg recommends nothing on `all`, and a udeb is skipped.
            BinaryPackageCase::from(vec![(
                "usr/bin/weak",
                elf(TestElf::executable().needed("libc.so.6")),
            )])
            .field("Architecture", "all"),
            amd64(
                "usr/bin/weak",
                elf(TestElf::executable().needed("libc.so.6")),
            )
            .udeb(),
            // Built with Go.
            amd64(
                "usr/bin/weak",
                elf(TestElf::executable().needed("libc.so.6")),
            )
            .package("golang-foo-bin"),
            amd64(
                "usr/bin/weak",
                elf(TestElf::executable().needed("libc.so.6")),
            )
            .field("Section", "golang"),
            amd64(
                "usr/bin/weak",
                elf(TestElf::executable().needed("libc.so.6")),
            )
            .field("Built-Using", "golang-1.21 (= 1.21.6-1)"),
        ];
        let fail = vec![
            amd64(
                "usr/bin/weak",
                elf(TestElf::executable().needed("libc.so.6")),
            ),
            amd64(
                "usr/bin/weak-link",
                TestFile::from_bytes(TestElf::executable().needed("libc.so.6").build()).hardlink(),
            ),
            amd64(
                "usr/bin/weak",
                elf(TestElf::executable().needed("libc.so.6")),
            )
            .field("Section", "contrib/golang"),
        ];
        BinaryPackageTester::new(HardeningNoPie, pass, fail).test_and_snapshot();
    }
}
