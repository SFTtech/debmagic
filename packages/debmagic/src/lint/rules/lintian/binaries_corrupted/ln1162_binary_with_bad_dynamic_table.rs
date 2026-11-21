use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct BinaryWithBadDynamicTable;

declare_rule! {
    /// An ELF file has a `PT_DYNAMIC` program header but no dynamic section in
    /// the file.
    ///
    /// This is what a debug file looks like when it was copied from a split-off
    /// debugging package into a place it does not belong: `PT_DYNAMIC` is
    /// there, with a `p_filesz` of zero, so `readelf` says "There is no
    /// dynamic section in this file". Walks the Data files ELF Entries
    /// (regular files and hardlinks, of any `e_type`) and skips paths under
    /// `usr/lib/debug/`. Lintian also reads the patched files of a Source tree
    /// with the same Check; only Binary packages are covered here. `elf-error`
    /// and `elf-warning`, which share the Check and quote `readelf`'s
    /// messages, are not implemented. Extra is empty; the Data files path is
    /// the Location.
    BinaryWithBadDynamicTable,
    code = "LN1162",
    tag = "binary-with-bad-dynamic-table",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BinaryWithBadDynamicTable {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, facts) in elf_files.iter() {
            if !facts.has_bad_dynamic_table() || path.starts_with("usr/lib/debug") {
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

    fn case(path: &str, elf: TestElf) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(elf.build()))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(
                "usr/lib/libfoo.so.1",
                TestElf::shared_object().needed("libc.so.6"),
            ),
            // A static executable has no dynamic segment at all.
            case("usr/bin/static", TestElf::executable()),
            // Debug files belong under `usr/lib/debug/`.
            case(
                "usr/lib/debug/usr/lib/libfoo.so.1",
                TestElf::shared_object().no_dynamic_content(),
            ),
            case(
                "usr/lib/debug/.build-id/ab/cdef.debug",
                TestElf::executable().no_dynamic_content(),
            ),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.dbg", TestFile::new("text"))]),
        ];
        let fail = vec![
            case(
                "usr/lib/libfoo.so.dbg",
                TestElf::shared_object().no_dynamic_content(),
            ),
            case(
                "usr/bin/hello.dbg",
                TestElf::executable().no_dynamic_content(),
            ),
            case(
                "usr/bin/hello",
                TestElf::pie_executable().no_dynamic_content(),
            ),
            // Only `usr/lib/debug/` is exempt.
            case(
                "usr/lib/debugging/x.so",
                TestElf::shared_object().no_dynamic_content(),
            ),
            case(
                "lib/debug/libfoo.so.1",
                TestElf::shared_object().no_dynamic_content(),
            ),
            BinaryPackageCase::from(vec![(
                "usr/bin/hello.dbg",
                TestFile::from_bytes(TestElf::executable().no_dynamic_content().build()).hardlink(),
            )]),
            case(
                "usr/lib/libfoo.so.dbg",
                TestElf::shared_object().no_dynamic_content(),
            )
            .udeb(),
        ];
        BinaryPackageTester::new(BinaryWithBadDynamicTable, pass, fail).test_and_snapshot();
    }
}
