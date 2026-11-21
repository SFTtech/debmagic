use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct SharedLibraryLacksStackSection;

declare_rule! {
    /// A shared library has no `PT_GNU_STACK` program header, so the loader
    /// assumes it needs an executable stack.
    ///
    /// A shared library is a Data files ELF Entry (regular file or hardlink)
    /// with a `DT_SONAME`. Skips packages with no `Architecture` field. A
    /// library whose `PT_GNU_STACK` asks for an executable stack is
    /// `executable-stack-in-shared-library`. Extra is empty; the Data files
    /// path is the Location.
    SharedLibraryLacksStackSection,
    code = "LN1155",
    tag = "shared-library-lacks-stack-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SharedLibraryLacksStackSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        // Lintian asks whether the package declares an `Architecture`.
        if ctx
            .installation_control()
            .fields()
            .get("Architecture")
            .is_none()
        {
            return;
        }
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, facts) in elf_files.iter() {
            if facts.soname().is_empty() || !(facts.stack_permissions().is_none()) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
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
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().stack(7)),
                "amd64",
            ),
            // Not a shared library: no `DT_SONAME`.
            case(
                "usr/bin/tool",
                TestFile::from_bytes(TestElf::executable().build()),
                "amd64",
            ),
            // No `Architecture` field.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", lib(TestElf::shared_object()))]),
            case("usr/lib/libfoo.so.1", TestFile::new("not an elf"), "amd64"),
        ];
        let fail = vec![
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object()),
                "amd64",
            ),
            case("usr/lib/libfoo.so.1", lib(TestElf::shared_object()), "all"),
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object()).hardlink(),
                "amd64",
            ),
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object()),
                "mips64el",
            ),
        ];
        BinaryPackageTester::new(SharedLibraryLacksStackSection, pass, fail).test_and_snapshot();
    }
}
