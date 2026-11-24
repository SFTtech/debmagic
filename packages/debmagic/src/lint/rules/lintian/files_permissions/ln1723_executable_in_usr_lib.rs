use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct ExecutableInUsrLib;

declare_rule! {
    /// An executable file is installed under `usr/lib/`.
    ///
    /// Walks Data files. A file or hardlink under `usr/lib/` with any execute
    /// bit set is reported; FHS places such executables in `usr/libexec/`.
    /// Symlinks and directories are not reported, and the Tag is skipped for
    /// udebs. Two lintian Screens mask the hint: files under
    /// `usr/lib/emacsen-common/packages/` when the binary's Pre-Depends and
    /// Depends satisfy `emacsen-common`, and scripts or ELF files under
    /// `usr/lib/cgi-bin/`. Extra is empty; the Location is the file.
    ExecutableInUsrLib,
    code = "LN1723",
    tag = "executable-in-usr-lib",
    default_selected = false,
    experimental = true,
}

impl BinaryPackageRule for ExecutableInUsrLib {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let emacsen = ctx.strong_relation().satisfies("emacsen-common");
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| is_executable_in_usr_lib(entry))
            .filter(|entry| !(emacsen && is_emacsen_package_file(entry)))
            .filter(|entry| !is_cgi_script_or_elf(files, entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

fn is_executable_in_usr_lib(entry: &Entry) -> bool {
    entry.path().starts_with("usr/lib") && entry.is_executable_file()
}

/// lintian's `Screen/Emacs/Elpa/Scripts.pm`: `^usr/lib/emacsen-common/packages/`.
fn is_emacsen_package_file(entry: &Entry) -> bool {
    entry.path().starts_with("usr/lib/emacsen-common/packages/")
}

/// lintian's `Screen/Web/Cgi/Scripts.pm`: a script or an ELF file under
/// `usr/lib/cgi-bin/`. The content checks are lintian's `is_script` (`#!`, but
/// not the lincity `#!#!#!` data) and `is_elf` (the `\x7fELF` magic).
fn is_cgi_script_or_elf(files: &FileTree, entry: &Entry) -> bool {
    if !entry.path().starts_with("usr/lib/cgi-bin/") {
        return false;
    }
    let Ok(bytes) = files.read(entry) else {
        return false;
    };
    let elf = bytes.starts_with(b"\x7fELF");
    let script = Hashbang::is_script(&bytes);
    elf || script
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn exec(content: &str) -> TestFile {
        TestFile::new(content).mode(0o755)
    }

    fn case(entries: Vec<(impl Into<String>, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .into_iter()
                .map(|(path, file)| (path.into(), file))
                .collect::<Vec<(String, TestFile)>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(Vec::<(String, TestFile)>::new()),
            // Executables belong in usr/libexec.
            case(vec![("usr/libexec/foo".to_string(), exec("x"))]),
            // Not executable.
            case(vec![(
                "usr/lib/libfoo.so.1".to_string(),
                TestFile::new("x").mode(0o644),
            )]),
            // Directories are not files, even with the execute bit.
            case(vec![(
                "usr/lib/foo".to_string(),
                TestFile::directory().mode(0o755),
            )]),
            // A symlink is not a file.
            case(vec![("usr/lib/foo".to_string(), TestFile::symlink("bar"))]),
            // Only usr/lib, not a similar prefix like usr/library.
            case(vec![("usr/library/foo".to_string(), exec("x"))]),
            case(vec![("lib/foo".to_string(), exec("x"))]),
            // Udebs are skipped.
            case(vec![("usr/lib/foo".to_string(), exec("x"))]).udeb(),
        ];
        let fail = vec![
            case(vec![("usr/lib/some-executable".to_string(), exec("x"))]),
            // A hardlink counts as a file, so it is reported too.
            case(vec![
                ("usr/lib/foo".to_string(), exec("x")),
                (
                    "usr/lib/bar".to_string(),
                    TestFile::new("x").mode(0o755).hardlink_to("usr/lib/foo"),
                ),
            ]),
            // Nested directories too.
            case(vec![("usr/lib/foo/bar/baz".to_string(), exec("x"))]),
        ];
        BinaryPackageTester::new(ExecutableInUsrLib, pass, fail).test_and_snapshot();
    }

    fn exec_bytes(content: &[u8]) -> TestFile {
        TestFile::from_bytes(content.to_vec()).mode(0o755)
    }

    /// lintian `Screen/Emacs/Elpa/Scripts.pm` masks the hint under
    /// `usr/lib/emacsen-common/packages/` when the strong relation (Pre-Depends
    /// and Depends) satisfies `emacsen-common`. Only a plain name in the strong
    /// relation counts; a Recommends, or an alternative, does not.
    #[test]
    fn emacsen_packages_are_masked_by_a_strong_emacsen_common_dependency() {
        let script = "#!/bin/sh\n";
        let pass = vec![
            case(vec![(
                "usr/lib/emacsen-common/packages/install/foo",
                exec_bytes(script.as_bytes()),
            )])
            .field("Depends", "emacsen-common"),
            case(vec![(
                "usr/lib/emacsen-common/packages/remove/foo",
                exec_bytes(script.as_bytes()),
            )])
            .field("Pre-Depends", "emacsen-common"),
            case(vec![(
                "usr/lib/emacsen-common/packages/install/foo",
                exec_bytes(script.as_bytes()),
            )])
            .field("Depends", "emacsen-common (>= 3.0)"),
        ];
        let fail = vec![
            // No strong dependency on emacsen-common.
            case(vec![(
                "usr/lib/emacsen-common/packages/install/foo",
                exec_bytes(script.as_bytes()),
            )]),
            // Recommends is not strong.
            case(vec![(
                "usr/lib/emacsen-common/packages/install/foo",
                exec_bytes(script.as_bytes()),
            )])
            .field("Recommends", "emacsen-common"),
            // An alternative does not satisfy the name.
            case(vec![(
                "usr/lib/emacsen-common/packages/install/foo",
                exec_bytes(script.as_bytes()),
            )])
            .field("Depends", "emacs | emacsen-common"),
            // Only the packages directory is masked.
            case(vec![(
                "usr/lib/emacsen-common/other/foo",
                exec_bytes(script.as_bytes()),
            )])
            .field("Depends", "emacsen-common"),
        ];
        BinaryPackageTester::new(ExecutableInUsrLib, pass, fail).test();
    }

    /// lintian `Screen/Web/Cgi/Scripts.pm` masks the hint under `usr/lib/cgi-bin/`
    /// for a script (a `#!` file, not a lincity `#!#!#!` data file) and for an
    /// ELF file. Other executables in that folder are still reported.
    #[test]
    fn cgi_bin_scripts_and_elf_files_are_masked() {
        let elf = b"\x7fELF\x02\x01\x01\x00".to_vec();
        let pass = vec![
            case(vec![(
                "usr/lib/cgi-bin/hello",
                exec_bytes(b"#!/bin/sh\necho hi\n"),
            )]),
            case(vec![("usr/lib/cgi-bin/hello", exec_bytes(&elf))]),
            // Nested folders below cgi-bin are masked too.
            case(vec![(
                "usr/lib/cgi-bin/sub/hello",
                exec_bytes(b"#!/usr/bin/perl\n"),
            )]),
        ];
        let fail = vec![
            // Neither a script nor ELF.
            case(vec![("usr/lib/cgi-bin/data", exec_bytes(b"plain data"))]),
            // lincity data files start with #!#!#! and are not scripts.
            case(vec![("usr/lib/cgi-bin/data", exec_bytes(b"#!#!#!\n"))]),
            // Only the cgi-bin folder is masked.
            case(vec![("usr/lib/cgi-bins/hello", exec_bytes(b"#!/bin/sh\n"))]),
            case(vec![("usr/lib/other/hello", exec_bytes(&elf))]),
        ];
        BinaryPackageTester::new(ExecutableInUsrLib, pass, fail).test();
    }

    /// lintian `Screen/Emacs/Elpa/Scripts.pm` is for the binary and udeb types;
    /// a binary without a strong relation to emacsen-common keeps its hint
    /// whatever the file's content.
    #[test]
    fn strong_relation_reads_pre_depends_and_depends_only() {
        let pass = vec![
            case(vec![(
                "usr/lib/emacsen-common/packages/x",
                exec_bytes(b"#!/bin/sh\n"),
            )])
            .field("Pre-Depends", "emacsen-common")
            .field("Depends", "foo"),
        ];
        let fail = vec![
            case(vec![(
                "usr/lib/emacsen-common/packages/x",
                exec_bytes(b"#!/bin/sh\n"),
            )])
            .field("Depends", "foo")
            .field("Suggests", "emacsen-common"),
        ];
        BinaryPackageTester::new(ExecutableInUsrLib, pass, fail).test();
    }
}
