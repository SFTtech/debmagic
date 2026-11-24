use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::ElfFacts;
use crate::lint::elf_symbol_tables::has_hardened_variant;
use crate::lint::hardening::{built_with_golang, built_with_rust, recommended_features_of};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct HardeningNoFortifyFunctions;

declare_rule! {
    /// An ELF object calls C functions that have a fortified `__<name>_chk`
    /// variant and calls none of the fortified variants.
    ///
    /// Walks the dynamic symbols of the Data files ELF Entries (regular files
    /// and hardlinks, of any `e_type`). Only undefined symbols count. A
    /// symbol `__<name>_chk` (or `_chkieee128`) is a hardened call when
    /// `<name>` is in lintian's `binaries/hardened-functions`; any other
    /// undefined symbol in that table is an unhardened one. A file is a hit
    /// when it has unhardened and no hardened calls. Reported only when dpkg's
    /// `hardening=+all` enables `fortify` on the package's `Architecture`
    /// (nothing for `all`), and the package was not built with Go (source or
    /// package name starting with `golang-`, `Section: golang`, or
    /// `Built-Using` naming `golang-N.N`) or Rust (`Built-Using` naming
    /// `rustc`). Lintian also counts a source package whose `Build-Depends-All`
    /// satisfies `golang-go | golang-any` or `rustc` from the same lint
    /// group; a Binary package Subject has no source package, so those cases
    /// are not skipped. Not skipped for udebs. Static-library archive members
    /// are not inspected. Info severity, experimental, and not selected by
    /// default, as in lintian. Extra is empty; the Data files path is the
    /// Location.
    HardeningNoFortifyFunctions,
    code = "LN1157",
    tag = "hardening-no-fortify-functions",
    default_selected = false,
    experimental = true,
}

impl BinaryPackageRule for HardeningNoFortifyFunctions {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !recommended_features_of(ctx).iter().any(|f| f == "fortify")
            || built_with_golang(ctx)
            || built_with_rust(ctx)
        {
            return;
        }
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, facts) in elf_files.iter() {
            if !unfortified(facts) {
                continue;
            }
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: path.to_path_buf(),
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// `__<name>_chk` or `__<name>_chkieee128`: the `<name>` it hardens.
fn hardened_name(symbol: &str) -> Option<&str> {
    let inner = symbol.strip_prefix("__")?;
    let name = inner
        .strip_suffix("_chk")
        .or_else(|| inner.strip_suffix("_chkieee128"))?;
    (!name.is_empty() && !name.contains(char::is_whitespace)).then_some(name)
}

fn unfortified(facts: &ElfFacts) -> bool {
    let mut hardened = false;
    let mut unhardened = false;
    for symbol in facts.symbols().iter().filter(|s| s.is_undefined()) {
        match hardened_name(symbol.name()) {
            Some(name) => hardened |= has_hardened_variant(name),
            None => unhardened |= has_hardened_variant(symbol.name()),
        }
    }
    unhardened && !hardened
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn amd64(path: &str, elf: TestElf) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(elf.build()))])
            .field("Architecture", "amd64")
    }

    fn calling(function: &str) -> TestElf {
        TestElf::executable()
            .needed("libc.so.6")
            .import(function, Some("GLIBC_2.2.5"))
    }

    #[test]
    fn binary() {
        let pass = vec![
            // The fortified variant is called, with or without the plain one.
            amd64("usr/bin/a", calling("__sprintf_chk")),
            amd64("usr/bin/a", calling("sprintf").import("__strcpy_chk", None)),
            // Not in the table of functions that have a hardened variant.
            amd64("usr/bin/a", calling("memcpy")),
            amd64("usr/bin/a", calling("puts")),
            // Nothing imported.
            amd64("usr/bin/a", TestElf::executable()),
            // No `fortify` for `all`, and no architecture at all.
            BinaryPackageCase::from(vec![(
                "usr/bin/a",
                TestFile::from_bytes(calling("sprintf").build()),
            )])
            .field("Architecture", "all"),
            BinaryPackageCase::from(vec![(
                "usr/bin/a",
                TestFile::from_bytes(calling("sprintf").build()),
            )]),
            amd64("usr/bin/a", calling("sprintf")).package("golang-foo-bin"),
            amd64("usr/bin/a", calling("sprintf")).field("Built-Using", "rustc (= 1.75)"),
            amd64("usr/bin/a", calling("sprintf")).field("Static-Built-Using", "rustc (= 1.75)"),
        ];
        let fail = vec![
            amd64("usr/bin/a", calling("sprintf")),
            amd64("usr/bin/a", calling("printf")),
            amd64("usr/bin/a", calling("strcpy").import("memcpy", None)),
            // A relocatable object and a udeb are not skipped.
            amd64("usr/lib/a.o", {
                TestElf::relocatable().import("sprintf", Some("GLIBC_2.2.5"))
            }),
            amd64("usr/bin/a", calling("sprintf")).udeb(),
            // Go is `golang-N.N`, not any name containing it.
            amd64("usr/bin/a", calling("sprintf")).field("Built-Using", "golang-go"),
            amd64("usr/bin/a", calling("sprintf")).field("Built-Using", "gcc-13 (= 13.2.0-1)"),
        ];
        BinaryPackageTester::new(HardeningNoFortifyFunctions, pass, fail).test_and_snapshot();
    }

    #[test]
    fn fortified_names() {
        assert_eq!(hardened_name("__sprintf_chk"), Some("sprintf"));
        assert_eq!(hardened_name("__vsnprintf_chkieee128"), Some("vsnprintf"));
        assert_eq!(hardened_name("sprintf"), None);
        assert_eq!(hardened_name("__chk"), None);
        assert_eq!(hardened_name("___chk"), None);
    }
}
