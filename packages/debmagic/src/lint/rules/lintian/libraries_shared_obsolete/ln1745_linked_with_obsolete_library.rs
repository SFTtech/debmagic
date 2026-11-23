use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct LinkedWithObsoleteLibrary;

declare_rule! {
    /// An ELF file has a `NEEDED` entry for the obsolete `libcblas.so.<N>`.
    ///
    /// The CBLAS symbols were merged into `libblas.so`, and `libcblas.so` is
    /// still shipped but is not managed by `update-alternatives`, so a binary
    /// that links it may run slowly. Walks the ELF facts of Data files. Each
    /// `NEEDED` entry that starts with `libcblas.so.` and a digit is a hint,
    /// with that entry as Extra and the file as the Location. Reported as an
    /// info hint. Udebs are checked too.
    LinkedWithObsoleteLibrary,
    code = "LN1745",
    tag = "linked-with-obsolete-library",
    default_selected = false,
    experimental = false,
}

static OBSOLETE_CBLAS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^libcblas\.so\.\d").expect("valid regex"));

impl BinaryPackageRule for LinkedWithObsoleteLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let hits: Vec<(std::path::PathBuf, String)> = elf_files
            .iter()
            .flat_map(|(path, facts)| {
                facts
                    .needed()
                    .iter()
                    .filter(|name| OBSOLETE_CBLAS.is_match(name))
                    .map(move |name| (path.to_path_buf(), name.clone()))
            })
            .collect();
        for (path, name) in hits {
            ctx.diagnostic(
                Diagnostic::warn(name)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
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

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // The non-obsolete BLAS and CBLAS-like names.
            BinaryPackageCase::from(vec![(
                "usr/bin/consumer",
                elf(TestElf::executable().needed("libblas.so.3")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/consumer",
                elf(TestElf::executable().needed("libcblas.so")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/consumer",
                elf(TestElf::executable().needed("libcblas.so.x")),
            )]),
            // Not an ELF file.
            BinaryPackageCase::from(vec![(
                "usr/bin/consumer",
                TestFile::new("libcblas.so.1\n").mode(0o755),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/bin/consumer",
                elf(TestElf::executable().needed("libcblas.so.1")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object()
                    .needed("libc.so.6")
                    .needed("libcblas.so.3")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/consumer",
                elf(TestElf::executable().needed("libcblas.so.1")),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(LinkedWithObsoleteLibrary, pass, fail).test_and_snapshot();
    }
}
