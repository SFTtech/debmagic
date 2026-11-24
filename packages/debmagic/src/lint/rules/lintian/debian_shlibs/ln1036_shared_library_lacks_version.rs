use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::lines::{elf_sonames, human_soname, in_ldconfig_folder};

pub struct SharedLibraryLacksVersion;

declare_rule! {
    /// A public shared library has an ELF SONAME with no version.
    ///
    /// The SONAME of each Data file is turned into `name version` the way
    /// dpkg-shlibdeps splits it. A SONAME that yields no space (no `.so.N`
    /// or `-N.so` suffix) cannot be represented in shlibs. Only files
    /// directly in a lintian `ldconfig_folders` entry are reported: the
    /// multiarch `lib/<triplet>` and `usr/lib/<triplet>` folders and the fixed
    /// `lib`, `usr/lib`, `usr/local/lib` and `*32`/`*64`/`*x32` folders.
    /// Private libraries elsewhere are quiet. Extra is `path pretty-soname`
    /// with no Location. Only ELF SONAMEs are read; a library without an
    /// SONAME is not reported. Symlinks are not ELF files and are quiet.
    SharedLibraryLacksVersion,
    code = "LN1036",
    tag = "shared-library-lacks-version",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SharedLibraryLacksVersion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf) = ctx.data_elf_files() else {
            return;
        };
        for (path, soname) in elf_sonames(&elf) {
            let pretty = human_soname(&soname);
            if pretty.contains(' ') || !in_ldconfig_folder(&path) {
                continue;
            }
            let extra = format!("{} {pretty}", path.to_string_lossy());
            ctx.diagnostic(Diagnostic::warn(extra));
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

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/libpriv.so",
                elf(TestElf::shared_object().soname("libpriv.so")),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/libbaz.so", elf(TestElf::shared_object()))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libbaz.so",
                TestFile::symlink("libbaz.so.1"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libbaz.so",
                TestFile::new("not an ELF file\n"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/libbaz.so",
                elf(TestElf::shared_object().soname("libbaz.so")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libbaz.so",
                elf(TestElf::shared_object().soname("libbaz.so")),
            )])
            .udeb(),
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/libbaz.so",
                elf(TestElf::shared_object().soname("libbaz.so")),
            )]),
        ];
        BinaryPackageTester::new(SharedLibraryLacksVersion, pass, fail).test_and_snapshot();
    }
}
