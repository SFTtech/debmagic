use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::lines::{
    elf_sonames, human_soname, in_ldconfig_folder, is_nss_plugin, read_text, shlibs_pretty_sonames,
};
use super::shlibs;

pub struct ShipsUndeclaredSharedLibrary;

declare_rule! {
    /// The Binary package ships a public shared library the shlibs control
    /// file does not declare.
    ///
    /// Runs only when the package has a regular `shlibs` file. For each ELF
    /// file with a versioned SONAME (`name version`) in a lintian
    /// `ldconfig_folders` entry, the `name version` key must appear as a
    /// non-udeb line of the shlibs file. The `libnss_*.so.N` plugins are
    /// skipped. Extra is `name version for path`; the Location is `shlibs`
    /// with no line. An unversioned SONAME is reported by LN1036 instead.
    ShipsUndeclaredSharedLibrary,
    code = "LN1039",
    tag = "ships-undeclared-shared-library",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ShipsUndeclaredSharedLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf) = ctx.data_elf_files() else {
            return;
        };
        let Some(text) = read_text(ctx, "shlibs") else {
            return;
        };
        let declared = shlibs_pretty_sonames(&text);
        for (path, soname) in elf_sonames(&elf) {
            let pretty = human_soname(&soname);
            if !pretty.contains(' ')
                || !in_ldconfig_folder(&path)
                || declared.contains(&pretty)
                || is_nss_plugin(&path)
            {
                continue;
            }
            let extra = format!("{pretty} for {}", path.to_string_lossy());
            ctx.diagnostic(Diagnostic::error(extra).with_location(shlibs::at()));
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
            library("usr/lib/libfoo.so.1", "libfoo.so.1")
                .control_file("shlibs", TestFile::new("libfoo 1 libfoo1\n")),
            library("usr/lib/libfoo.so.1", "libfoo.so.1"),
            library("usr/lib/private/libp.so.1", "libp.so.1")
                .control_file("shlibs", TestFile::new("libother 1\n")),
            library("usr/lib/libfoo.so.1", "libfoo.so.1")
                .control_file("shlibs", TestFile::symlink("other")),
            library("usr/lib/libnss_files.so.2", "libnss_files.so.2")
                .control_file("shlibs", TestFile::new("libother 1\n")),
            library("usr/lib/libbaz.so", "libbaz.so")
                .control_file("shlibs", TestFile::new("libother 1\n")),
        ];
        let fail = vec![
            library("usr/lib/libeshf.so.1", "libeshf.so.1")
                .control_file("shlibs", TestFile::new("libother 1\n")),
            library("usr/lib/libfoo.so.1", "libfoo.so.1")
                .control_file("shlibs", TestFile::new("udeb: libfoo 1 libfoo1\n")),
            library("usr/lib/libfoo.so.1", "libfoo.so.1")
                .control_file("shlibs", TestFile::new("libfoo 2 libfoo1\n")),
        ];
        BinaryPackageTester::new(ShipsUndeclaredSharedLibrary, pass, fail).test_and_snapshot();
    }
}
