use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct EmacsenCommonWithoutDhElpa;

declare_rule! {
    /// The package uses emacsen-common but was not built with dh-elpa.
    ///
    /// Reports when the Data files have the directory
    /// `usr/lib/emacsen-common/packages/install/` and not
    /// `usr/share/emacs/site-lisp/elpa-src/`. A directory counts when it is
    /// present as an entry or has any entry below it. Extra is empty. Udebs are
    /// checked too.
    EmacsenCommonWithoutDhElpa,
    code = "LN0838",
    tag = "emacsen-common-without-dh-elpa",
    default_selected = true,
    experimental = false,
}

const EMACSEN_INSTALL: &str = "usr/lib/emacsen-common/packages/install";
const ELPA_SRC: &str = "usr/share/emacs/site-lisp/elpa-src";

impl BinaryPackageRule for EmacsenCommonWithoutDhElpa {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let has_directory = |dir: &str| {
            let below = format!("{dir}/");
            files.entries().iter().any(|entry| {
                entry.path().to_str().is_some_and(|path| {
                    path.starts_with(&below)
                        || (path == dir && entry.kind() == EntryKind::Directory)
                })
            })
        };
        if has_directory(EMACSEN_INSTALL) && !has_directory(ELPA_SRC) {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            // Without the emacsen-common install hooks.
            tree(vec![("usr/share/emacs/site-lisp/x", TestFile::directory())]),
            // Built with dh-elpa.
            tree(vec![
                (
                    "usr/lib/emacsen-common/packages/install",
                    TestFile::directory(),
                ),
                ("usr/share/emacs/site-lisp/elpa-src", TestFile::directory()),
            ]),
            tree(vec![
                (
                    "usr/lib/emacsen-common/packages/install/foo",
                    TestFile::new("#!/bin/sh\n"),
                ),
                (
                    "usr/share/emacs/site-lisp/elpa-src/foo/foo.el",
                    TestFile::new(";; x\n"),
                ),
            ]),
            // A plain file is not the install directory.
            tree(vec![(
                "usr/lib/emacsen-common/packages/install",
                TestFile::new("x"),
            )]),
        ];
        let fail = vec![
            tree(vec![(
                "usr/lib/emacsen-common/packages/install",
                TestFile::directory(),
            )]),
            tree(vec![(
                "usr/lib/emacsen-common/packages/install/foo",
                TestFile::new("#!/bin/sh\n"),
            )]),
            tree(vec![
                (
                    "usr/lib/emacsen-common/packages/install/foo",
                    TestFile::new("#!/bin/sh\n"),
                ),
                ("usr/share/emacs/site-lisp/x.el", TestFile::new(";; x\n")),
            ]),
            tree(vec![(
                "usr/lib/emacsen-common/packages/install/foo",
                TestFile::new("#!/bin/sh\n"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(EmacsenCommonWithoutDhElpa, pass, fail).test_and_snapshot();
    }
}
