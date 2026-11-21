use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::{ElfFacts, ElfFiles, ElfObjectType};
use crate::lint::hardening::built_with_golang;
use crate::lint::rule::BinaryPackageRule;

pub struct StaticallyLinkedBinary;

declare_rule! {
    /// The Binary package installs a statically linked ELF executable.
    ///
    /// Walks the ELF facts of Data files (regular files and hardlinks) that
    /// `file(1)` would call an executable: `ET_EXEC`, or `ET_DYN` with
    /// `DF_1_PIE` (static-pie included). Shared objects and relocatable
    /// objects are never hits. An executable is a hit when its dynamic
    /// section has no `NEEDED`. Skips udebs; paths under `boot/` or
    /// `usr/lib/debug/`, ending in `-static` or `.static`, and
    /// `sbin/ldconfig`; packages whose name ends in `-static`; a `PT_INTERP`
    /// matching `/lib/klibc-*.so`; and packages built with Go (source or
    /// package name starting with `golang-`, `Section: golang`, or
    /// `Built-Using` / `Static-Built-Using` naming `golang-N.N`). Lintian
    /// also counts a source package whose `Build-Depends-All` satisfies
    /// `golang-go | golang-any` when that source is in the same lint group; a
    /// Binary package Subject has no source package, so that case is not
    /// skipped. Extra is empty; the Data files path is the Location.
    StaticallyLinkedBinary,
    code = "LN0703",
    tag = "statically-linked-binary",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for StaticallyLinkedBinary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() || ctx.package_name().ends_with("-static") || built_with_golang(ctx) {
            return;
        }
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for path in static_paths(&elf_files) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn static_paths(elf_files: &ElfFiles) -> Vec<PathBuf> {
    elf_files
        .iter()
        .filter(|(path, facts)| is_static(facts) && !excluded_path(path))
        .map(|(path, _)| path.to_path_buf())
        .collect()
}

fn is_static(facts: &ElfFacts) -> bool {
    facts.object_type() == ElfObjectType::Executable
        && facts.needed().is_empty()
        && !facts
            .interpreter()
            .is_some_and(|interpreter| KLIBC_INTERPRETER.is_match(interpreter))
}

fn excluded_path(path: &Path) -> bool {
    let Some(name) = path.to_str() else {
        return false;
    };
    name.starts_with("boot/")
        || name.ends_with("-static")
        || name.ends_with(".static")
        || name.starts_with("usr/lib/debug/")
        || name == "sbin/ldconfig"
}

static KLIBC_INTERPRETER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/lib/klibc-\S+\.so").expect("klibc interpreter regex"));

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn static_exe() -> TestFile {
        elf(TestElf::executable())
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/bin/dynamic",
                elf(TestElf::pie_executable()
                    .interpreter("/lib64/ld-linux-x86-64.so.2")
                    .needed("libc.so.6")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/nopie",
                elf(TestElf::executable().needed("libc.so.6")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/foo.o", elf(TestElf::relocatable()))]),
            BinaryPackageCase::from(vec![("usr/bin/script", TestFile::new("#!/bin/sh\n"))]),
            BinaryPackageCase::from(vec![("usr/bin/link", TestFile::symlink("static"))]),
            BinaryPackageCase::from(vec![("usr/bin/static", static_exe())]).udeb(),
            BinaryPackageCase::from(vec![("boot/loader", static_exe())]),
            BinaryPackageCase::from(vec![("usr/bin/busybox-static", static_exe())]),
            BinaryPackageCase::from(vec![("usr/bin/hello.static", static_exe())]),
            BinaryPackageCase::from(vec![("usr/lib/debug/usr/bin/hello", static_exe())]),
            BinaryPackageCase::from(vec![("sbin/ldconfig", static_exe())]),
            BinaryPackageCase::from(vec![("usr/bin/static", static_exe())])
                .package("busybox-static"),
            BinaryPackageCase::from(vec![(
                "usr/lib/klibc/bin/sh",
                elf(TestElf::executable().interpreter("/lib/klibc-AbC123.so")),
            )]),
            BinaryPackageCase::from(vec![("usr/bin/tool", static_exe())])
                .field("Source", "golang-github-foo (1.0-1)"),
            BinaryPackageCase::from(vec![("usr/bin/tool", static_exe())]).package("golang-foo-bin"),
            BinaryPackageCase::from(vec![("usr/bin/tool", static_exe())])
                .field("Section", "golang"),
            BinaryPackageCase::from(vec![("usr/bin/tool", static_exe())])
                .field("Built-Using", "golang-1.21 (= 1.21.6-1)"),
            BinaryPackageCase::from(vec![("usr/bin/tool", static_exe())])
                .field("Static-Built-Using", "golang-1.22 (= 1.22.0-1)"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/bin/static", static_exe())]),
            BinaryPackageCase::from(vec![("usr/bin/hello", elf(TestElf::pie_executable()))]),
            BinaryPackageCase::from(vec![("usr/bin/static-hello", static_exe().hardlink())]),
            BinaryPackageCase::from(vec![(
                "usr/bin/elfobject",
                elf(TestElf::executable().missing_section_headers()),
            )]),
            // A debug-only copy: the interpreter and dynamic segments are empty.
            BinaryPackageCase::from(vec![(
                "usr/bin/hello.dbg",
                elf(TestElf::executable().interpreter("")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/other-interpreter",
                elf(TestElf::executable().interpreter("/lib/ld-musl-x86_64.so.1")),
            )]),
            BinaryPackageCase::from(vec![("lib/debug/hello", static_exe())]),
            BinaryPackageCase::from(vec![("usr/bin/tool", static_exe())])
                .field("Section", "contrib/golang"),
            BinaryPackageCase::from(vec![("usr/bin/tool", static_exe())])
                .field("Built-Using", "gcc-13 (= 13.2.0-1)"),
        ];
        BinaryPackageTester::new(StaticallyLinkedBinary, pass, fail).test_and_snapshot();
    }
}
