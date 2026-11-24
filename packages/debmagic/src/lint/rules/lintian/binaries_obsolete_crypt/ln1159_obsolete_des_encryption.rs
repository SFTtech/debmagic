use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf_symbol_tables::obsolete_crypt_tag;
use crate::lint::rule::BinaryPackageRule;

pub struct ObsoleteDesEncryption;

declare_rule! {
    /// An ELF object imports a `libcrypt` function that implies the obsolete
    /// DES encryption algorithm: `encrypt`, `encrypt_r`, `setkey`, or
    /// `setkey_r`.
    ///
    /// Walks the dynamic symbols of the Data files ELF Entries (regular files
    /// and hardlinks, of any `e_type`); only undefined symbols count. The
    /// table is lintian's `binaries/obsolete-crypt-functions`. One Diagnostic
    /// per symbol and file; extra is the symbol name and the Data files path
    /// is the Location. Archive members of static libraries are not
    /// inspected, and udebs are not skipped.
    ObsoleteDesEncryption,
    code = "LN1159",
    tag = "obsolete-des-encryption",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ObsoleteDesEncryption {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, facts) in elf_files.iter() {
            for symbol in facts.symbols().iter().filter(|s| s.is_undefined()) {
                if obsolete_crypt_tag(symbol.name()) != Some("obsolete-des-encryption") {
                    continue;
                }
                ctx.diagnostic(Diagnostic::error(symbol.name()).with_location(Location {
                    path: path.to_path_buf(),
                    line: None,
                    column: None,
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(path: &str, elf: TestElf) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(elf.build()))])
    }

    fn importing(names: &[&str]) -> TestElf {
        names.iter().fold(
            TestElf::executable().needed("libcrypt.so.1"),
            |elf, name| elf.import(name, Some("XCRYPT_2.0")),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            case("usr/bin/a", importing(&["crypt", "crypt_r"])),
            // Defining the function is not using it: no imports at all.
            case("usr/bin/a", TestElf::executable()),
            // The alias is the other Tag.
            case("usr/bin/a", importing(&["fcrypt"])),
            BinaryPackageCase::from(vec![("usr/bin/a", TestFile::new("setkey"))]),
        ];
        let fail = vec![
            case("usr/bin/a", importing(&["setkey"])),
            case("usr/bin/a", importing(&["encrypt", "setkey_r", "crypt"])),
            case("usr/bin/a", importing(&["encrypt_r"])).udeb(),
            case(
                "usr/lib/libfoo.so.1",
                TestElf::shared_object()
                    .soname("libfoo.so.1")
                    .import("encrypt", None),
            ),
        ];
        BinaryPackageTester::new(ObsoleteDesEncryption, pass, fail).test_and_snapshot();
    }
}
