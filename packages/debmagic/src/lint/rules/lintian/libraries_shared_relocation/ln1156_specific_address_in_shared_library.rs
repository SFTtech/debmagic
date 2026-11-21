use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct SpecificAddressInSharedLibrary;

declare_rule! {
    /// A shared library is not position independent: its dynamic section has
    /// a `DT_TEXTREL` entry, so the loader must patch its text.
    ///
    /// A shared library is a Data files ELF Entry (regular file or hardlink)
    /// with a `DT_SONAME`. `DT_FLAGS` `DF_TEXTREL` alone is not a hit, as in
    /// lintian, which reads only the `TEXTREL` dynamic entry. Extra is empty;
    /// the Data files path is the Location.
    SpecificAddressInSharedLibrary,
    code = "LN1156",
    tag = "specific-address-in-shared-library",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SpecificAddressInSharedLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, facts) in elf_files.iter() {
            if facts.soname().is_empty() || !(facts.has_textrel()) {
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
                lib(TestElf::shared_object()),
                "amd64",
            ),
            // Not a shared library: no `DT_SONAME`.
            case(
                "usr/bin/tool",
                TestFile::from_bytes(TestElf::executable().textrel().build()),
                "amd64",
            ),
            case("usr/lib/libfoo.so.1", TestFile::new("not an elf"), "amd64"),
        ];
        let fail = vec![
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().textrel()),
                "i386",
            ),
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().textrel().stack(6)),
                "amd64",
            ),
            case(
                "usr/lib/libfoo.so.1",
                lib(TestElf::shared_object().textrel()).hardlink(),
                "amd64",
            ),
        ];
        BinaryPackageTester::new(SpecificAddressInSharedLibrary, pass, fail).test_and_snapshot();
    }
}
