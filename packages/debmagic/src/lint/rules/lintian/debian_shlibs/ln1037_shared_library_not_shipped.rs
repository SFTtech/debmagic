use std::collections::BTreeSet;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::lines::{elf_sonames, human_soname, read_text, shlibs_pretty_sonames};
use super::shlibs;

pub struct SharedLibraryNotShipped;

declare_rule! {
    /// The shlibs control file declares a library the Binary package does not
    /// ship.
    ///
    /// Runs only when the package ships at least one ELF SONAME with a version
    /// (`name version`) and has a regular `shlibs` file. Each shlibs key is
    /// `name version`, or `udeb: name version` on a udeb line. A key is used
    /// when a versioned SONAME gives the same `name version`, with and without
    /// the `udeb: ` prefix. Each unused key is reported with the shlibs
    /// Location (no line), in sorted order. Extra is the key as written.
    /// Symlinked or hardlinked shlibs are read as lintian reads them: a
    /// symlink is not a file, so it is skipped.
    SharedLibraryNotShipped,
    code = "LN1037",
    tag = "shared-library-not-shipped",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SharedLibraryNotShipped {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf) = ctx.data_elf_files() else {
            return;
        };
        let mut used = BTreeSet::new();
        for (_, soname) in elf_sonames(&elf) {
            let pretty = human_soname(&soname);
            if pretty.contains(' ') {
                used.insert(format!("udeb: {pretty}"));
                used.insert(pretty);
            }
        }
        if used.is_empty() {
            return;
        }
        let Some(shlibs_text) = read_text(ctx, "shlibs") else {
            return;
        };
        for key in shlibs_pretty_sonames(&shlibs_text).difference(&used) {
            ctx.diagnostic(Diagnostic::warn(key.clone()).with_location(shlibs::at()));
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

    fn shlibs(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/lib/libfoo.so.1",
            elf(TestElf::shared_object().soname("libfoo.so.1")),
        )])
        .control_file("shlibs", TestFile::new(body))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            shlibs("libfoo 1 libfoo1\n"),
            shlibs("# comment\n\nlibfoo 1\n"),
            shlibs("udeb: libfoo 1 libfoo1\n"),
            shlibs("").control_file("shlibs", TestFile::symlink("other")),
            BinaryPackageCase::from(vec![(
                "usr/lib/libbaz.so",
                elf(TestElf::shared_object().soname("libbaz.so")),
            )])
            .control_file("shlibs", TestFile::new("libgone 1\n")),
        ];
        let fail = vec![
            shlibs("libgone 1\nlibfoo 1\n"),
            shlibs("udeb: libgone 1 pkg\nlibfoo 1\n"),
            shlibs("libfoo 1\nlibgone 2\n").udeb(),
        ];
        BinaryPackageTester::new(SharedLibraryNotShipped, pass, fail).test_and_snapshot();
    }
}
