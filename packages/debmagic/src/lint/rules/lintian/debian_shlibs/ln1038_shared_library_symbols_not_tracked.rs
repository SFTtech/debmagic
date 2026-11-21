use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::lines::{
    elf_sonames, human_soname, in_ldconfig_folder, is_nss_plugin, symbols_sonames, symbols_text,
};
use super::symbols;

pub struct SharedLibrarySymbolsNotTracked;

declare_rule! {
    /// A public shared library has no entry in the symbols control file.
    ///
    /// Runs only when the package has a `symbols` control file (any kind;
    /// a non-regular file or invalid UTF-8 reads as empty, as lintian does).
    /// For each ELF file with an SONAME in a lintian `ldconfig_folders` entry,
    /// the SONAME must be named by a line of the symbols file. The
    /// `libnss_*.so.N` plugins are skipped. Extra is `name version for path`
    /// with the SONAME turned into `name version` as the Check does. The
    /// Location is `symbols` with no line. The symbols file is compared by
    /// its raw SONAME text, not by a `name version` key.
    SharedLibrarySymbolsNotTracked,
    code = "LN1038",
    tag = "shared-library-symbols-not-tracked",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SharedLibrarySymbolsNotTracked {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf) = ctx.data_elf_files() else {
            return;
        };
        let Some(text) = symbols_text(ctx) else {
            return;
        };
        let tracked = symbols_sonames(&text);
        for (path, soname) in elf_sonames(&elf) {
            if !in_ldconfig_folder(&path) || tracked.contains(&soname) || is_nss_plugin(&path) {
                continue;
            }
            let extra = format!("{} for {}", human_soname(&soname), path.to_string_lossy());
            ctx.diagnostic(Diagnostic::warn(extra).with_location(symbols::at(None)));
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

    fn library(path: &str, soname: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, elf(TestElf::shared_object().soname(soname)))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            library("usr/lib/libesym.so.1", "libesym.so.1").control_file(
                "symbols",
                TestFile::new("libesym.so.1 libesym1 #MINVER#\n foo 1.0\n"),
            ),
            library("usr/lib/libesym.so.1", "libesym.so.1"),
            library("usr/lib/private/libp.so.1", "libp.so.1"),
            library("usr/lib/libnss_files.so.2", "libnss_files.so.2"),
        ];
        let fail = vec![
            library("usr/lib/libesym.so.1", "libesym.so.1")
                .control_file("symbols", TestFile::from_bytes(vec![0xff, 0xfe, b'\n'])),
            library("usr/lib/libesym.so.1", "libesym.so.1")
                .control_file("symbols", TestFile::new("libbaz.so.2 libbaz1\n foo 1.0\n")),
            library("usr/lib/libesym.so.1", "libesym.so.1")
                .control_file("symbols", TestFile::new("libbaz.so.2 libbaz1\n foo 1.0\n"))
                .udeb(),
            library("usr/lib/libbaz.so", "libbaz.so")
                .control_file("symbols", TestFile::new("libbaz.so.2 libbaz1\n")),
        ];
        BinaryPackageTester::new(SharedLibrarySymbolsNotTracked, pass, fail).test_and_snapshot();
    }
}
