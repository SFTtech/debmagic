use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf_symbol_tables::obsolete_crypt_tag;
use crate::lint::rule::BinaryPackageRule;

pub struct ObsoleteCryptAlias;

declare_rule! {
    /// An ELF object imports `fcrypt`, an alternative, less portable name for
    /// `crypt`.
    ///
    /// Walks the dynamic symbols of the Data files ELF Entries (regular files
    /// and hardlinks, of any `e_type`); only undefined symbols count. The
    /// table is lintian's `binaries/obsolete-crypt-functions`. Extra is the
    /// symbol name and the Data files path is the Location. Archive members
    /// of static libraries are not inspected, and udebs are not skipped.
    ObsoleteCryptAlias,
    code = "LN1160",
    tag = "obsolete-crypt-alias",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ObsoleteCryptAlias {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, facts) in elf_files.iter() {
            for symbol in facts.symbols().iter().filter(|s| s.is_undefined()) {
                if obsolete_crypt_tag(symbol.name()) != Some("obsolete-crypt-alias") {
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
            case("usr/bin/a", importing(&["crypt"])),
            case("usr/bin/a", importing(&["setkey", "encrypt"])),
            case("usr/bin/a", TestElf::executable()),
            BinaryPackageCase::from(vec![("usr/bin/a", TestFile::new("fcrypt"))]),
        ];
        let fail = vec![
            case("usr/bin/a", importing(&["fcrypt"])),
            case("usr/bin/a", importing(&["crypt", "fcrypt"])).udeb(),
        ];
        BinaryPackageTester::new(ObsoleteCryptAlias, pass, fail).test_and_snapshot();
    }
}
