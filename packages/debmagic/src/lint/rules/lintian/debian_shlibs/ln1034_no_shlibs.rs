use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::shlibs;

pub struct NoShlibs;

declare_rule! {
    /// A public versioned shared library ships without a shlibs control file.
    ///
    /// One finding per versioned shared library (an ELF SONAME with a version)
    /// whose directory is one of lintian's `ldconfig_folders`, when the package
    /// has no `shlibs` control file. A `shlibs` that is a symlink counts as
    /// absent. Skipped for udebs and for libc NSS plugins (`libnss_<name>.so.<n>`).
    /// Extra is the library's path (no `[pointer]`, so no Location); a path that is
    /// not UTF-8 is shown lossily.
    NoShlibs,
    code = "LN1034",
    tag = "no-shlibs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NoShlibs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() || shlibs::control_text(ctx).is_some() {
            return;
        }
        for path in shlibs::versioned_libraries(ctx) {
            if shlibs::in_ldconfig_folder(&path) && !shlibs::is_nss_plugin(&path) {
                ctx.diagnostic(Diagnostic::error(path.display().to_string()));
            }
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

    fn library(path: &'static str, soname: &str) -> (&'static str, TestFile) {
        (path, elf(TestElf::shared_object().soname(soname)))
    }

    #[test]
    fn test() {
        let pass = vec![
            // The shlibs control file is present.
            BinaryPackageCase::from(vec![library("usr/lib/libfoo.so.1", "libfoo.so.1")])
                .control_file("shlibs", TestFile::new("libfoo 1 libfoo1\n")),
            // A udeb is not checked.
            BinaryPackageCase::from(vec![library("usr/lib/libfoo.so.1", "libfoo.so.1")]).udeb(),
            // Not in a library directory, or only in a subdirectory of one.
            BinaryPackageCase::from(vec![library("usr/share/libfoo.so.1", "libfoo.so.1")]),
            BinaryPackageCase::from(vec![library("usr/lib/foo/libfoo.so.1", "libfoo.so.1")]),
            // Unversioned: no version in the SONAME.
            BinaryPackageCase::from(vec![library("usr/lib/libfoo.so", "libfoo.so")]),
            // A libc NSS plugin.
            BinaryPackageCase::from(vec![library(
                "usr/lib/libnss_files.so.2",
                "libnss_files.so.2",
            )]),
            BinaryPackageCase::from(vec![library(
                "lib/x86_64-linux-gnu/libnss_files.so.2",
                "libnss_files.so.2",
            )]),
            // Without a SONAME it is not a shared library for this Rule.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", elf(TestElf::shared_object()))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![library("usr/lib/libfoo.so.1", "libfoo.so.1")]),
            BinaryPackageCase::from(vec![library("lib/libfoo.so.1", "libfoo.so.1")]),
            BinaryPackageCase::from(vec![library("usr/local/lib/libfoo.so.1", "libfoo.so.1")]),
            BinaryPackageCase::from(vec![library(
                "usr/lib/x86_64-linux-gnu/libfoo.so.1",
                "libfoo.so.1",
            )]),
            // A dash and digits in the SONAME is a version too.
            BinaryPackageCase::from(vec![library("usr/lib/libfoo-1.2.so", "libfoo-1.2.so")]),
            // A shlibs that is a symlink counts as absent.
            BinaryPackageCase::from(vec![library("usr/lib/libfoo.so.1", "libfoo.so.1")])
                .control_file("shlibs", TestFile::symlink("other")),
            // Several libraries: one finding each, in path order.
            BinaryPackageCase::from(vec![
                library("usr/lib/libbar.so.2", "libbar.so.2"),
                library("usr/lib/libfoo.so.1", "libfoo.so.1"),
            ]),
        ];
        BinaryPackageTester::new(NoShlibs, pass, fail).test_and_snapshot();
    }
}
