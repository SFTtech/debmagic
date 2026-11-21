use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::ElfFacts;
use crate::lint::rule::BinaryPackageRule;

pub struct BinaryCompiledWithProfilingEnabled;

declare_rule! {
    /// An ELF object imports a glibc profiling hook: `__fentry__`, or
    /// `mcount`, `_mcount`, `__mcount`, `gnu_mcount_nc`, `__gnu_mcount_nc`
    /// and their relatives (`_?_?(gnu_)?mcount(_nc)?`).
    ///
    /// Walks the dynamic symbols of the Data files ELF Entries (regular files
    /// and hardlinks, of any `e_type`). The symbol must be undefined and need
    /// a `GLIBC_` version. Lintian also accepts a symbol in the section named
    /// `.text`, but its symbol table never resolves section numbers to
    /// names, so only undefined symbols ever match; so here. Not skipped for
    /// udebs. Extra is empty; the Data files path is the Location.
    BinaryCompiledWithProfilingEnabled,
    code = "LN1158",
    tag = "binary-compiled-with-profiling-enabled",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BinaryCompiledWithProfilingEnabled {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, facts) in elf_files.iter() {
            if !is_profiled(facts) {
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

static MCOUNT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A_{0,2}(gnu_)?mcount(_nc)?\z").expect("mcount regex"));

fn is_profiled(facts: &ElfFacts) -> bool {
    facts.symbols().iter().any(|symbol| {
        symbol.is_undefined()
            && symbol.version().starts_with("GLIBC_")
            && (symbol.name() == "__fentry__" || MCOUNT.is_match(symbol.name()))
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(path: &str, elf: TestElf) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(elf.build()))])
    }

    fn importing(name: &str, version: Option<&str>) -> TestElf {
        TestElf::executable()
            .needed("libc.so.6")
            .import(name, version)
    }

    #[test]
    fn binary() {
        let pass = vec![
            case("usr/bin/a", importing("printf", Some("GLIBC_2.2.5"))),
            // The version must be glibc's.
            case("usr/bin/a", importing("__fentry__", None)),
            case("usr/bin/a", importing("mcount", Some("LIBFOO_1.0"))),
            // Names that merely contain the hook.
            case("usr/bin/a", importing("my_mcount", Some("GLIBC_2.2.5"))),
            case("usr/bin/a", importing("___mcount", Some("GLIBC_2.2.5"))),
            case("usr/bin/a", importing("__fentry__x", Some("GLIBC_2.2.5"))),
            case("usr/bin/a", TestElf::executable()),
            BinaryPackageCase::from(vec![("usr/bin/a", TestFile::new("mcount"))]),
        ];
        let fail = vec![
            case("usr/bin/a", importing("__fentry__", Some("GLIBC_2.13"))),
            case("usr/bin/a", importing("mcount", Some("GLIBC_2.2.5"))),
            case("usr/bin/a", importing("_mcount", Some("GLIBC_2.2.5"))),
            case("usr/bin/a", importing("__mcount", Some("GLIBC_2.2.5"))),
            case("usr/bin/a", importing("__gnu_mcount_nc", Some("GLIBC_2.4"))),
            case("usr/bin/a", importing("gnu_mcount_nc", Some("GLIBC_2.4"))),
            case(
                "usr/lib/libfoo.so.1",
                TestElf::shared_object()
                    .soname("libfoo.so.1")
                    .import("mcount", Some("GLIBC_2.2.5")),
            ),
            case("usr/bin/a", importing("mcount", Some("GLIBC_2.2.5"))).udeb(),
        ];
        BinaryPackageTester::new(BinaryCompiledWithProfilingEnabled, pass, fail)
            .test_and_snapshot();
    }
}
