use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::shared::objects;

pub struct SharedLibraryLacksPrerequisites;

declare_rule! {
    /// A shared object has no `NEEDED` entries at all.
    ///
    /// Reported at each ELF shared object (`ET_DYN` without `DF_1_PIE`) with
    /// no `NEEDED` entry, except the files lintian excludes: kernel and syslinux
    /// modules (`boot/modules/`, `lib/modules/`), detached debugging information
    /// (`usr/lib/debug/`), `.c32` and `.e64` files, Java `*.debuginfo` files under
    /// `usr/lib/jvm/`, Guile object files (`usr/lib/.../guile/<ver>/*.go`), and
    /// the dynamic loader `ld-<version>.so` directly under `lib/`, `lib32/`,
    /// `libx32/`, or `lib64/` (or one directory down under them). Skipped for
    /// udebs. The Location is the file.
    ///
    /// Where we differ from lintian: none. Lintian's
    /// `Screen/Coq/Cmxs/Prerequisites.pm` masks the hint for any `*.cmxs` file
    /// in a binary or udeb package (OCaml native plugins, whose undefined
    /// symbols are expected); so does this Rule. Udebs never reach that mask
    /// here because they are skipped above, as in lintian.
    SharedLibraryLacksPrerequisites,
    code = "LN1298",
    tag = "shared-library-lacks-prerequisites",
    default_selected = true,
    experimental = false,
}

static GUILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib(?:/[^/]+)+/guile/[^/]+/.+\.go$").expect("valid regex"));

static LOADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^lib(?:|32|x32|64)/(?:[-\w/]+/)?ld-[\d.]+\.so$").expect("valid regex")
});

static KERNEL_OR_BOOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:boot|lib)/modules/").expect("valid regex"));

static EXCLUDED_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:[ce]32|e64)$").expect("valid regex"));

static JAVA_DEBUGINFO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/jvm/.*\.debuginfo$").expect("valid regex"));

/// Files lintian's `shared-library-lacks-prerequisites` does not report.
fn excluded(name: &str) -> bool {
    name.ends_with(".cmxs")
        || KERNEL_OR_BOOT.is_match(name)
        || name.starts_with("usr/lib/debug/")
        || EXCLUDED_SUFFIX.is_match(name)
        || JAVA_DEBUGINFO.is_match(name)
        || GUILE.is_match(name)
        || LOADER.is_match(name)
}

impl BinaryPackageRule for SharedLibraryLacksPrerequisites {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let paths: Vec<PathBuf> = objects(ctx)
            .into_iter()
            .filter(|object| object.shared && object.needed.is_empty())
            .filter(|object| !excluded(&object.path.to_string_lossy()))
            .map(|object| object.path)
            .collect();
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

    fn shared(needed: &[&str]) -> TestFile {
        let mut elf = TestElf::shared_object();
        for library in needed {
            elf = elf.needed(library);
        }
        TestFile::from_bytes(elf.build())
    }

    fn case(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files).field("Architecture", "amd64")
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(vec![("usr/lib/libfoo.so.1", shared(&["libc.so.6"]))]),
            // A program with no NEEDED is a static program, not a shared object.
            case(vec![(
                "usr/bin/foo",
                TestFile::from_bytes(TestElf::executable().build()),
            )]),
            // Excluded paths.
            case(vec![("lib/modules/6.1/foo.ko", shared(&[]))]),
            case(vec![("boot/modules/foo.c32", shared(&[]))]),
            case(vec![("usr/lib/debug/usr/lib/libfoo.so.1", shared(&[]))]),
            case(vec![(
                "usr/lib/jvm/java-17/lib/libfoo.debuginfo",
                shared(&[]),
            )]),
            case(vec![("usr/lib/x/guile/3.0/ccache/foo.go", shared(&[]))]),
            case(vec![("lib/x86_64-linux-gnu/ld-2.31.so", shared(&[]))]),
            case(vec![("lib64/ld-2.31.so", shared(&[]))]),
            // Relocatable objects are not visited.
            case(vec![(
                "usr/lib/foo.o",
                TestFile::from_bytes(TestElf::relocatable().build()),
            )]),
            // A udeb is not checked.
            case(vec![("usr/lib/libfoo.so.1", shared(&[]))]).udeb(),
        ];
        let fail = vec![
            case(vec![("usr/lib/libfoo.so.1", shared(&[]))]),
            case(vec![
                ("usr/lib/libfoo.so.1", shared(&[])),
                ("usr/lib/libbar.so.1", shared(&[])),
            ]),
            // Only the ld-*.so pattern under the loader directories is excluded.
            case(vec![("usr/lib/ld-2.31.so", shared(&[]))]),
            case(vec![("usr/lib/libfoo.e32x", shared(&[]))]),
        ];
        BinaryPackageTester::new(SharedLibraryLacksPrerequisites, pass, fail).test_and_snapshot();
    }

    /// lintian `Screen/Coq/Cmxs/Prerequisites.pm` masks the hint for any file
    /// whose name ends in `.cmxs` in a binary or udeb package: such files are
    /// ELF shared objects with undefined symbols by design. Only the name's
    /// suffix counts, so `.cmxs.so`, `.cmx` and a bare `cmxs` are still reported.
    #[test]
    fn cmxs_files_are_masked_and_nothing_else_is_masked() {
        let pass = vec![
            case(vec![("usr/lib/ocaml/foo/foo.cmxs", shared(&[]))]),
            case(vec![("usr/lib/ocaml/foo.cmxs", shared(&[]))]),
            // Masked in udebs too (udebs are skipped by this Rule anyway).
            case(vec![("usr/lib/ocaml/foo/foo.cmxs", shared(&[]))]).udeb(),
        ];
        let fail = vec![
            // Neighbours with the same shared-object content are still reported.
            case(vec![("usr/lib/ocaml/foo/foo.cmxs.so", shared(&[]))]),
            case(vec![("usr/lib/ocaml/foo/foo.cmx", shared(&[]))]),
            case(vec![("usr/lib/ocaml/foo/foocmxs", shared(&[]))]),
            // The mask is per file: an unmasked library beside a .cmxs is reported.
            case(vec![
                ("usr/lib/ocaml/foo/foo.cmxs", shared(&[])),
                ("usr/lib/libfoo.so.1", shared(&[])),
            ]),
        ];
        BinaryPackageTester::new(SharedLibraryLacksPrerequisites, pass, fail).test();
    }
}
