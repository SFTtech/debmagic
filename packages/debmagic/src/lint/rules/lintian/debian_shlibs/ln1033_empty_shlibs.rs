use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::shlibs;

pub struct EmptyShlibs;

declare_rule! {
    /// The shlibs control file exists but the package ships no versioned shared library.
    ///
    /// A versioned shared library is a Data file with an ELF SONAME that has a
    /// version (`libfoo.so.1`, `libfoo-1.2.so`); an unversioned SONAME
    /// (`libfoo.so`) does not count. The `shlibs` control file must be a regular
    /// file or hardlink, a symlink counts as absent. Extra is empty and the
    /// Location is `shlibs`. The directory of the library does not matter, and a
    /// udeb is checked like a deb.
    EmptyShlibs,
    code = "LN1033",
    tag = "empty-shlibs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for EmptyShlibs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if shlibs::control_text(ctx).is_none() {
            return;
        }
        if !shlibs::versioned_libraries(ctx).is_empty() {
            return;
        }
        ctx.diagnostic(Diagnostic::error("").with_location(shlibs::at()));
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

    fn shlibs() -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("shlibs", TestFile::new("libfoo 1 libfoo1\n"))
    }

    #[test]
    fn test() {
        let pass = vec![
            // No shlibs file: nothing to be empty.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .control_file("shlibs", TestFile::new("libfoo 1 libfoo1\n")),
            // A versioned SONAME with a dash and digits.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo-1.2.so",
                elf(TestElf::shared_object().soname("libfoo-1.2.so")),
            )])
            .control_file("shlibs", TestFile::new("libfoo 1.2 libfoo1\n")),
            // The library need not sit in a library directory.
            BinaryPackageCase::from(vec![(
                "opt/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .control_file("shlibs", TestFile::new("libfoo 1 libfoo1\n")),
            // A shlibs that is a symlink counts as absent.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("shlibs", TestFile::symlink("other")),
            // A udeb with a versioned library is not empty either.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .control_file("shlibs", TestFile::new("libfoo 1 libfoo1\n"))
            .udeb(),
        ];
        let fail = vec![
            shlibs(),
            // A shlibs as a hardlink is a file and so counts.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("shlibs", TestFile::new("libfoo 1 libfoo1\n").hardlink()),
            // A library with no SONAME does not make the shlibs file non-empty.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so", elf(TestElf::shared_object()))])
                .control_file("shlibs", TestFile::new("libfoo 1 libfoo1\n")),
            // An unversioned SONAME is not a versioned library.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so",
                elf(TestElf::shared_object().soname("libfoo.so")),
            )])
            .control_file("shlibs", TestFile::new("libfoo 1 libfoo1\n")),
            // A shlibs with no ELF object at all.
            BinaryPackageCase::from(vec![("usr/bin/tool", TestFile::new("#!/bin/sh\n"))])
                .control_file("shlibs", TestFile::new("libfoo 1 libfoo1\n")),
        ];
        BinaryPackageTester::new(EmptyShlibs, pass, fail).test_and_snapshot();
    }
}
