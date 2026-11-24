use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::{ElfClass, ElfEndianness, ElfFacts, ElfFiles};
use crate::lint::rule::BinaryPackageRule;

pub struct BinaryFromOtherArchitecture;

declare_rule! {
    /// An ELF object in a Binary package is for another processor than the
    /// package's `Architecture`: `ld` and `strip` would not work on it, and it
    /// will not run here.
    ///
    /// Walks the ELF facts of Data files (regular files and hardlinks) and
    /// skips a package whose `Architecture` is `all`. An object matches an
    /// architecture when its `ELFCLASS`, byte order and `e_machine` are the
    /// ones of that architecture (a table transcribed from lintian's
    /// `binaries/arch-regex`, with ARM and SPARC variants accepted together).
    /// Files are not reported when they match the architecture; when the
    /// path is in a `lib32/`, `lib64/` or `libx32/` multi-arch directory or
    /// in `emul/ia32`, the bus width matches; when the object is a
    /// detached debugging file under `usr/lib/debug/.build-id/`, a Guile
    /// object, or a kernel module under `lib/modules/` of an i386 or amd64
    /// `Architecture` with the 64-bit equivalent present; and an i386 object
    /// in an amd64 package is not reported. Extra is empty; the Data files
    /// path is the Location. Not skipped for udebs, like lintian.
    ///
    /// Lintian's `file(1)` signature regexes are replaced by the object facts,
    /// so the operating system part (`GNU/Linux`, `GNU/Hurd`, `kFreeBSD`) and
    /// the ARM and SPARC ABI details are not checked. An unknown
    /// `Architecture` matches nothing, as in lintian.
    BinaryFromOtherArchitecture,
    code = "LN1291",
    tag = "binary-from-other-architecture",
    default_selected = true,
    experimental = false,
}

/// What an object must be to match an architecture.
struct Signature {
    class: ElfClass,
    endianness: ElfEndianness,
    machines: &'static [u16],
}

const EM_386: u16 = 3;
const EM_MIPS: u16 = 8;
const EM_ARM: u16 = 40;
const EM_X86_64: u16 = 62;
const EM_ALPHA: u16 = 0x9026;

/// Architectures of lintian's `binaries/arch-regex`, by Debian name.
const ARCHITECTURES: &[(&str, Signature)] = &[
    (
        "alpha",
        sig(ElfClass::Elf64, ElfEndianness::Little, &[EM_ALPHA]),
    ),
    (
        "amd64",
        sig(ElfClass::Elf64, ElfEndianness::Little, &[EM_X86_64]),
    ),
    (
        "arm",
        sig(ElfClass::Elf32, ElfEndianness::Little, &[EM_ARM]),
    ),
    ("arm64", sig(ElfClass::Elf64, ElfEndianness::Little, &[183])),
    ("armeb", sig(ElfClass::Elf32, ElfEndianness::Big, &[EM_ARM])),
    (
        "armel",
        sig(ElfClass::Elf32, ElfEndianness::Little, &[EM_ARM]),
    ),
    (
        "armhf",
        sig(ElfClass::Elf32, ElfEndianness::Little, &[EM_ARM]),
    ),
    ("hppa", sig(ElfClass::Elf32, ElfEndianness::Big, &[15])),
    ("hppa64", sig(ElfClass::Elf64, ElfEndianness::Big, &[15])),
    (
        "hurd-amd64",
        sig(ElfClass::Elf64, ElfEndianness::Little, &[EM_X86_64]),
    ),
    (
        "hurd-i386",
        sig(ElfClass::Elf32, ElfEndianness::Little, &[EM_386]),
    ),
    (
        "i386",
        sig(ElfClass::Elf32, ElfEndianness::Little, &[EM_386]),
    ),
    ("ia64", sig(ElfClass::Elf64, ElfEndianness::Little, &[50])),
    (
        "kfreebsd-amd64",
        sig(ElfClass::Elf64, ElfEndianness::Little, &[EM_X86_64]),
    ),
    (
        "kfreebsd-i386",
        sig(ElfClass::Elf32, ElfEndianness::Little, &[EM_386]),
    ),
    (
        "loong64",
        sig(ElfClass::Elf64, ElfEndianness::Little, &[258]),
    ),
    (
        "lpia",
        sig(ElfClass::Elf32, ElfEndianness::Little, &[EM_386]),
    ),
    ("m32r", sig(ElfClass::Elf32, ElfEndianness::Big, &[88])),
    ("m68k", sig(ElfClass::Elf32, ElfEndianness::Big, &[4])),
    ("mips", sig(ElfClass::Elf32, ElfEndianness::Big, &[EM_MIPS])),
    (
        "mipsel",
        sig(ElfClass::Elf32, ElfEndianness::Little, &[EM_MIPS]),
    ),
    (
        "mips64",
        sig(ElfClass::Elf64, ElfEndianness::Big, &[EM_MIPS]),
    ),
    (
        "mips64el",
        sig(ElfClass::Elf64, ElfEndianness::Little, &[EM_MIPS]),
    ),
    ("powerpc", sig(ElfClass::Elf32, ElfEndianness::Big, &[20])),
    (
        "powerpcspe",
        sig(ElfClass::Elf32, ElfEndianness::Big, &[20]),
    ),
    ("ppc64", sig(ElfClass::Elf64, ElfEndianness::Big, &[21])),
    (
        "ppc64el",
        sig(ElfClass::Elf64, ElfEndianness::Little, &[21]),
    ),
    (
        "riscv64",
        sig(ElfClass::Elf64, ElfEndianness::Little, &[243]),
    ),
    ("s390", sig(ElfClass::Elf32, ElfEndianness::Big, &[22])),
    ("s390x", sig(ElfClass::Elf64, ElfEndianness::Big, &[22])),
    ("sh4", sig(ElfClass::Elf32, ElfEndianness::Little, &[42])),
    ("sparc", sig(ElfClass::Elf32, ElfEndianness::Big, &[2, 18])),
    ("sparc64", sig(ElfClass::Elf64, ElfEndianness::Big, &[43])),
    (
        "x32",
        sig(ElfClass::Elf32, ElfEndianness::Little, &[EM_X86_64]),
    ),
];

/// The phony "bus width" architectures: only the class is checked.
const BUS_WIDTHS: &[(&str, ElfClass)] = &[("32", ElfClass::Elf32), ("64", ElfClass::Elf64)];

/// `binaries/arch-64bit-equivs`: a 32-bit architecture and its 64-bit twin.
const EQUIVALENTS: &[(&str, &str)] = &[
    ("hppa", "hppa64"),
    ("i386", "amd64"),
    ("kfreebsd-i386", "kfreebsd-amd64"),
    ("mips", "mips64"),
    ("mipsel", "mipsel64"),
    ("powerpc", "ppc64"),
    ("s390", "s390x"),
    ("sparc", "sparc64"),
];

const fn sig(class: ElfClass, endianness: ElfEndianness, machines: &'static [u16]) -> Signature {
    Signature {
        class,
        endianness,
        machines,
    }
}

static BUS_WIDTH_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|/)lib(x?\d\d)/|^emul/ia(\d\d)").expect("bus width regex"));
static GUILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib(?:/[^/]+)+/guile/[^/]+/.+\.go$").expect("guile regex"));

impl BinaryPackageRule for BinaryFromOtherArchitecture {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let architecture = ctx
            .installation_control()
            .fields()
            .unfolded_value("Architecture")
            .unwrap_or_default();
        if architecture == "all" {
            return;
        }
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for path in from_other_architecture(&elf_files, &architecture) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// The paths of the ELF Entries that are not for `architecture`, in path order.
fn from_other_architecture(elf_files: &ElfFiles, architecture: &str) -> Vec<std::path::PathBuf> {
    elf_files
        .iter()
        .filter(|(path, facts)| is_from_other_architecture(path, facts, architecture))
        .map(|(path, _)| path.to_path_buf())
        .collect()
}

fn is_from_other_architecture(path: &Path, facts: &ElfFacts, architecture: &str) -> bool {
    if matches_architecture(facts, architecture) {
        return false;
    }

    // Old multi-arch directories name the bus width of their objects.
    let name = path.to_string_lossy();
    if let Some(captures) = BUS_WIDTH_DIR.captures(&name) {
        let width = captures
            .get(1)
            .or_else(|| captures.get(2))
            .map_or("", |width| width.as_str());
        if matches_architecture(facts, width) {
            return false;
        }
    }

    // Detached debugging symbols could be for a biarch library.
    if name.starts_with("usr/lib/debug/.build-id/") {
        return false;
    }

    // Guile binaries do not objdump/strip correctly.
    if GUILE.is_match(&name) {
        return false;
    }

    // Kernel modules of 32-bit architectures may be 64-bit objects.
    if name.starts_with("lib/modules/")
        && EQUIVALENTS
            .iter()
            .find(|(arch, _)| *arch == architecture)
            .is_some_and(|(_, equivalent)| matches_architecture(facts, equivalent))
    {
        return false;
    }

    // i386 objects are allowed in amd64 packages for now.
    if architecture == "amd64" && matches_architecture(facts, "i386") {
        return false;
    }

    true
}

/// Whether `facts` match the named architecture or bus width. Unknown names
/// match nothing.
fn matches_architecture(facts: &ElfFacts, architecture: &str) -> bool {
    if let Some((_, class)) = BUS_WIDTHS.iter().find(|(name, _)| *name == architecture) {
        return facts.class() == *class;
    }
    let Some((_, signature)) = ARCHITECTURES.iter().find(|(name, _)| *name == architecture) else {
        return false;
    };
    facts.class() == signature.class
        && facts.endianness() == signature.endianness
        && signature.machines.contains(&facts.machine())
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::{ElfClass, ElfEndianness, TestElf, header_only};
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn x86_64(path: &str) -> (&str, TestFile) {
        (path, TestFile::from_bytes(TestElf::executable().build()))
    }

    fn i386(path: &str) -> (&str, TestFile) {
        (
            path,
            TestFile::from_bytes(header_only(ElfClass::Elf32, ElfEndianness::Little, EM_386)),
        )
    }

    fn arm(path: &str) -> (&str, TestFile) {
        (
            path,
            TestFile::from_bytes(header_only(ElfClass::Elf32, ElfEndianness::Little, EM_ARM)),
        )
    }

    fn arm64(path: &str) -> (&str, TestFile) {
        (
            path,
            TestFile::from_bytes(header_only(ElfClass::Elf64, ElfEndianness::Little, 183)),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Architecture", "amd64"),
            // `all` is not checked.
            BinaryPackageCase::from(vec![arm("usr/bin/foo")]).field("Architecture", "all"),
            // The object is for the package's architecture.
            BinaryPackageCase::from(vec![x86_64("usr/bin/foo")]).field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![i386("usr/bin/foo")]).field("Architecture", "i386"),
            BinaryPackageCase::from(vec![arm64("usr/lib/libfoo.so")])
                .field("Architecture", "arm64"),
            // i386 objects are allowed in amd64 packages.
            BinaryPackageCase::from(vec![i386("usr/lib/libfoo.so")]).field("Architecture", "amd64"),
            // Multi-arch directories name the bus width.
            BinaryPackageCase::from(vec![i386("usr/lib32/libfoo.so")])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![arm("emul/ia32-linux/bin/foo")])
                .field("Architecture", "amd64"),
            // Detached debugging symbols of a biarch library.
            BinaryPackageCase::from(vec![arm("usr/lib/debug/.build-id/ab/cd.debug")])
                .field("Architecture", "amd64"),
            // Guile objects do not objdump correctly.
            BinaryPackageCase::from(vec![arm("usr/lib/foo/guile/2.2/fake-guile-object.go")])
                .field("Architecture", "amd64"),
            // Kernel modules: the 64-bit equivalent of an i386 package.
            BinaryPackageCase::from(vec![x86_64("lib/modules/6.1/foo.ko")])
                .field("Architecture", "i386"),
            // Not an ELF object, or not a file.
            BinaryPackageCase::from(vec![("usr/bin/foo.sh", TestFile::new("#!/bin/sh\n"))])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/bin/link", TestFile::symlink("/usr/bin/foo"))])
                .field("Architecture", "amd64"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![arm("usr/bin/foo")]).field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![x86_64("usr/bin/foo")]).field("Architecture", "arm64"),
            BinaryPackageCase::from(vec![arm64("usr/bin/foo")]).field("Architecture", "amd64"),
            // An i386 object in an i386-less package is not allowed for amd64 either.
            BinaryPackageCase::from(vec![i386("usr/bin/foo")]).field("Architecture", "armhf"),
            // A 64-bit object in a 32-bit multi-arch directory.
            BinaryPackageCase::from(vec![x86_64("usr/lib32/libfoo.so")])
                .field("Architecture", "i386"),
            // A hardlink counts as a file, like lintian's `is_file`.
            BinaryPackageCase::from(vec![(
                "usr/bin/foo-link",
                TestFile::from_bytes(TestElf::executable().build()).hardlink(),
            )])
            .field("Architecture", "arm64"),
            // An unknown architecture matches nothing.
            BinaryPackageCase::from(vec![x86_64("usr/bin/foo")]).field("Architecture", "any"),
            // udebs are not skipped.
            BinaryPackageCase::from(vec![arm("usr/bin/foo")])
                .field("Architecture", "amd64")
                .udeb(),
        ];
        BinaryPackageTester::new(BinaryFromOtherArchitecture, pass, fail).test_and_snapshot();
    }
}
