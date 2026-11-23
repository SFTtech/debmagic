use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

pub struct ElfWarning;

declare_rule! {
    /// An ELF file makes `readelf` print a warning. Experimental, so it is not
    /// selected by default. Lintian 2.144.0 reports this, but debmagic does
    /// not, so this Rule reports nothing.
    ///
    /// The lintian Check `binaries/corrupted` reports every `WARNINGS` entry
    /// that `Index::Elf` collects from `readelf: Warning:` output lines, for
    /// the file and for each static-library member. debmagic's ELF reader
    /// parses the headers itself and never produces such lines, so there is
    /// nothing to port yet. The Rule would need readelf's warnings, or a reader
    /// that reproduces them, to fire. Extra would be readelf's message as
    /// `In <section>: <text>` and the Location the file.
    ElfWarning,
    code = "LN1460",
    tag = "elf-warning",
    default_selected = false,
    experimental = true,
}

impl BinaryPackageRule for ElfWarning {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {}
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
        // Well-formed ELF objects, a hardlink, a non-ELF file and a udeb all
        // pass: no readelf warning is produced for any of them.
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("usr/bin/hello", elf(TestElf::executable()))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/hello",
                elf(TestElf::executable().import("puts", None)),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/libhello.so", elf(TestElf::shared_object()))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/hello-link",
                elf(TestElf::executable()).hardlink(),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/hello.txt",
                TestFile::new("not an ELF object\n"),
            )]),
            BinaryPackageCase::from(vec![("usr/bin/hello", elf(TestElf::executable()))]).udeb(),
        ];
        let fail = Vec::<BinaryPackageCase>::new();
        BinaryPackageTester::new(ElfWarning, pass, fail).test();
    }
}
