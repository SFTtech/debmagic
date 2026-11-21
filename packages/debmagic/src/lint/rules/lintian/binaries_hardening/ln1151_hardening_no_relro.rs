use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::hardening::lacking_hardening;
use crate::lint::rule::BinaryPackageRule;

pub struct HardeningNoRelro;

declare_rule! {
    /// A dynamically linked executable or shared object has no `PT_GNU_RELRO`
    /// program header.
    ///
    /// Covers the Data files ELF Entries (regular files and hardlinks) that
    /// `file(1)` calls an executable (`ET_EXEC`, or `ET_DYN` with `DF_1_PIE`)
    /// or a shared object and that have a `NEEDED` entry. Reported only when
    /// dpkg's `hardening=+all` enables `relro` on the package's `Architecture`
    /// (nothing for `all`). Skips udebs and packages built with Go (source or
    /// package name starting with `golang-`, `Section: golang`, or
    /// `Built-Using` naming `golang-N.N`). Lintian also counts a source
    /// package whose `Build-Depends-All` satisfies `golang-go | golang-any`
    /// from the same lint group; a Binary package Subject has no source
    /// package, so that case is not skipped. Extra is empty; the Data files
    /// path is the Location.
    HardeningNoRelro,
    code = "LN1151",
    tag = "hardening-no-relro",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for HardeningNoRelro {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(paths) = lacking_hardening(ctx, "relro", |facts| !facts.has_relro()) else {
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
                elf(TestElf::executable().needed("libc.so.6").relro()),
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
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object()
                    .soname("libfoo.so.1")
                    .needed("libc.so.6")),
            ),
            amd64(
                "usr/bin/weak",
                elf(TestElf::executable().needed("libc.so.6")),
            )
            .field("Section", "contrib/golang"),
        ];
        BinaryPackageTester::new(HardeningNoRelro, pass, fail).test_and_snapshot();
    }
}
