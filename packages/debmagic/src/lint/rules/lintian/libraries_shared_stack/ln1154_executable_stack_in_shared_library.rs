use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ExecutableStackInSharedLibrary;

declare_rule! {
    /// A shared library's `PT_GNU_STACK` program header asks for an executable
    /// stack.
    ///
    /// A shared library is a Data files ELF Entry (regular file or hardlink)
    /// with a `DT_SONAME`. It is a hit when it has a `PT_GNU_STACK` whose
    /// permissions are not `rw-` (`readelf` flags `RWE`, `RE`, ...). Skips
    /// packages whose `Architecture` contains `mips`, as lintian does
    /// (Debian bugs 1025436 and 1022787). A library with no `PT_GNU_STACK` is
    /// `shared-library-lacks-stack-section`. Extra is empty; the Data files
    /// path is the Location.
    ExecutableStackInSharedLibrary,
    code = "LN1154",
    tag = "executable-stack-in-shared-library",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ExecutableStackInSharedLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let mips = ctx
            .installation_control()
            .fields()
            .get("Architecture")
            .is_some_and(|architecture| architecture.contains("mips"));
        if mips {
            return;
        }
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, facts) in elf_files.iter() {
            if facts.soname().is_empty()
                || !(facts
                    .stack_permissions()
                    .is_some_and(|permissions| permissions != "rw-"))
            {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: path.to_path_buf(),
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

    fn lib(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.soname("libfoo.so.1").build())
    }

    fn case(path: &str, file: TestFile, architecture: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)]).field("Architecture", architecture)
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().stack(6)),
                "amd64",
            ),
            // No `PT_GNU_STACK` is the other Tag.
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object()),
                "amd64",
            ),
            // Not a shared library: no `DT_SONAME`.
            case(
                "usr/bin/tool",
                TestFile::from_bytes(TestElf::executable().stack(7).build()),
                "amd64",
            ),
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().stack(7)),
                "mips64el",
            ),
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().stack(7)),
                "mipsel",
            ),
            case("usr/lib/libfoo.so.1", TestFile::new("not an elf"), "amd64"),
        ];
        let fail = vec![
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().stack(7)),
                "amd64",
            ),
            // Read and execute, no write.
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().stack(5)),
                "amd64",
            ),
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().stack(7)),
                "all",
            ),
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().stack(7)).hardlink(),
                "amd64",
            ),
            // `mips` must be part of the architecture name.
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().stack(7)),
                "arm64",
            ),
        ];
        BinaryPackageTester::new(ExecutableStackInSharedLibrary, pass, fail).test_and_snapshot();
    }
}
