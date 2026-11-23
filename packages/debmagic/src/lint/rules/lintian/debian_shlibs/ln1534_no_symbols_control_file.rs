use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::elf::ElfSymbol;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::lines::{in_ldconfig_folder, is_nss_plugin};

pub struct NoSymbolsControlFile;

declare_rule! {
    /// A public shared library ships in a binary package that has no
    /// `symbols` control file, so `dpkg` cannot generate symbol-based
    /// dependencies for it. Info severity; not selected by default.
    ///
    /// Skipped for udebs, and when the package has a `symbols` control file
    /// of any kind. For each ELF Data file with an SONAME in a lintian
    /// `ldconfig_folders` entry, not a `libnss_*.so.N` plugin, the Rule
    /// reports unless some dynamic symbol that is undefined or defined in
    /// `.text` starts with `__objc_` (Objective-C libraries do not list their
    /// methods). Extra is the file's path; there is no Location, the Subject
    /// is the location.
    NoSymbolsControlFile,
    code = "LN1534",
    tag = "no-symbols-control-file",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for NoSymbolsControlFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let has_symbols_file = ctx
            .control_files()
            .is_some_and(|files| files.get("symbols").is_some());
        if has_symbols_file {
            return;
        }
        let Some(elf) = ctx.data_elf_files() else {
            return;
        };
        for (path, facts) in elf.iter() {
            if facts.soname().is_empty()
                || !in_ldconfig_folder(path)
                || is_nss_plugin(path)
                || facts.symbols().iter().any(is_objc_method)
            {
                continue;
            }
            ctx.diagnostic(
                Diagnostic::warn(path.to_string_lossy().into_owned()).with_severity(Severity::Info),
            );
        }
    }
}

/// A symbol the Check looks at (`.text` or undefined) whose name starts with
/// `__objc_`.
fn is_objc_method(symbol: &ElfSymbol) -> bool {
    matches!(symbol.section(), ".text" | "UND") && symbol.name().starts_with("__objc_")
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
            // A symbols control file of any kind is present.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .control_file("symbols", TestFile::new("libfoo.so.1 libfoo1 #MINVER#\n")),
            // A udeb is not checked.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .udeb(),
            // Not in an ldconfig folder.
            BinaryPackageCase::from(vec![(
                "usr/share/foo/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )]),
            // No SONAME: not a shared library for the Check.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so", elf(TestElf::shared_object()))]),
            // The libnss plugins are skipped.
            BinaryPackageCase::from(vec![(
                "usr/lib/libnss_files.so.2",
                elf(TestElf::shared_object().soname("libnss_files.so.2")),
            )]),
            // Objective-C methods: a `__objc_` symbol in .text or undefined.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object()
                    .soname("libfoo.so.1")
                    .sized_section(".text")
                    .defined("__objc_class_name_Foo", ".text")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object()
                    .soname("libfoo.so.1")
                    .import("__objc_msgSend", None)),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )]),
            // A `__objc_` symbol outside .text is not one the Check looks at.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object()
                    .soname("libfoo.so.1")
                    .sized_section(".data")
                    .defined("__objc_class_name_Foo", ".data")),
            )]),
            // A hardlink counts as a file.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")).hardlink(),
            )]),
        ];
        BinaryPackageTester::new(NoSymbolsControlFile, pass, fail).test_and_snapshot();
    }
}
