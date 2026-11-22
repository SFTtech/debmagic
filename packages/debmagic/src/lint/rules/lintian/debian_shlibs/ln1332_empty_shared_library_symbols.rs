use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::lines::elf_sonames;
use super::symbols;

pub struct EmptySharedLibrarySymbols;

declare_rule! {
    /// The Binary package has a `symbols` control file but ships no shared
    /// library.
    ///
    /// A shared library is any Data file with an ELF SONAME, versioned or
    /// not, as lintian's `soname_by_filename` counts them. Reported when the
    /// control file `symbols` exists (as an entry of any kind) and no such
    /// file is shipped. Applies to deb, udeb and ddeb alike. Extra is empty;
    /// the location is `symbols` without a line. A Binary package whose data
    /// files cannot be read is not reported.
    EmptySharedLibrarySymbols,
    code = "LN1332",
    tag = "empty-shared-library-symbols",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for EmptySharedLibrarySymbols {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let has_symbols_file = ctx
            .control_files()
            .is_some_and(|files| files.get("symbols").is_some());
        if !has_symbols_file {
            return;
        }
        let Some(elf) = ctx.data_elf_files() else {
            return;
        };
        if !elf_sonames(&elf).is_empty() {
            return;
        }
        ctx.diagnostic(Diagnostic::error("").with_location(symbols::at(None)));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn symbols_file() -> TestFile {
        TestFile::new("libfoo.so.1 libfoo1 #MINVER#\n | libfoo1 1\n")
    }

    fn shared_object(soname: &str) -> (&'static str, TestFile) {
        (
            "usr/lib/libfoo.so.1",
            TestFile::from_bytes(TestElf::shared_object().soname(soname).build()),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // A shared library with a SONAME, with the symbols file.
            BinaryPackageCase::from(vec![shared_object("libfoo.so.1")])
                .control_file("symbols", symbols_file()),
            // An unversioned SONAME still counts as a shared library.
            BinaryPackageCase::from(vec![shared_object("libfoo.so")])
                .control_file("symbols", symbols_file()),
            // No symbols file: nothing to report.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("shlibs", TestFile::new("libfoo 1\n")),
        ];
        let fail = vec![
            // Symbols file without any shared library.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("symbols", symbols_file()),
            // A library without SONAME is not a shared library.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                TestFile::from_bytes(TestElf::shared_object().build()),
            )])
            .control_file("symbols", symbols_file()),
            // udeb is not exempt.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("symbols", symbols_file())
                .udeb(),
        ];
        BinaryPackageTester::new(EmptySharedLibrarySymbols, pass, fail).test_and_snapshot();
    }
}
