use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct DevelopmentPackageShipsElfBinaryInPath;

declare_rule! {
    /// A development package (`Section` ending in `libdevel`, not
    /// `Multi-Arch: foreign`) ships an ELF binary in a program directory.
    ///
    /// Walks Data files that are regular files or hardlinks and are ELF (any
    /// `ELF` file, as the ELF facts see it). A hit is an ELF file whose parent
    /// directory is one of `bin/`, `sbin/`, `usr/bin/`, `usr/sbin/`, or
    /// `usr/games/`. Extra is empty; the Data files path is the Location.
    /// Severity is info and the Tag is not selected by default, as in lintian.
    DevelopmentPackageShipsElfBinaryInPath,
    code = "LN1294",
    tag = "development-package-ships-elf-binary-in-path",
    default_selected = false,
    experimental = false,
}

const PATH_DIRECTORIES: [&str; 5] = ["bin/", "sbin/", "usr/bin/", "usr/sbin/", "usr/games/"];

impl BinaryPackageRule for DevelopmentPackageShipsElfBinaryInPath {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let (section, multi_arch) = {
            let fields = ctx.installation_control().fields();
            (
                fields.get("Section").unwrap_or_default().trim().to_string(),
                fields
                    .get("Multi-Arch")
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            )
        };
        let is_libdevel = section == "libdevel" || section.ends_with("/libdevel");
        if !is_libdevel || multi_arch == "foreign" {
            return;
        }

        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| elf_files.get(entry.path()).is_some())
            .filter(|entry| {
                let dirname = entry
                    .path()
                    .parent()
                    .map(|parent| format!("{}/", parent.to_string_lossy()))
                    .unwrap_or_default();
                PATH_DIRECTORIES.contains(&dirname.as_str())
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(
                Diagnostic::warn("")
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

    fn elf() -> TestFile {
        TestFile::from_bytes(TestElf::executable().build())
    }

    fn libdevel(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
            .field("Architecture", "amd64")
            .field("Section", "libdevel")
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Not a development package.
            BinaryPackageCase::from(vec![("usr/bin/foo", elf())])
                .field("Architecture", "amd64")
                .field("Section", "utils"),
            // Multi-Arch: foreign is exempt.
            libdevel("usr/bin/foo", elf()).field("Multi-Arch", "foreign"),
            // Not an ELF file, or not in a program directory.
            libdevel("usr/bin/script", TestFile::new("#!/bin/sh\n")),
            libdevel("usr/lib/foo", elf()),
            libdevel("usr/bin/sub/foo", elf()),
            libdevel("usr/share/foo/bin", elf()),
            // A symbolic link is not a regular file.
            libdevel("usr/bin/link", TestFile::symlink("/usr/lib/foo")),
        ];
        let fail = vec![
            libdevel("usr/bin/foo", elf()),
            libdevel("bin/foo", elf()),
            libdevel("sbin/foo", elf()),
            libdevel("usr/sbin/foo", elf()),
            libdevel("usr/games/foo", elf()),
            // Multi-Arch: same is not foreign.
            libdevel("usr/bin/foo", elf()).field("Multi-Arch", "same"),
            // The Section may carry an area.
            BinaryPackageCase::from(vec![("usr/bin/foo", elf())])
                .field("Architecture", "amd64")
                .field("Section", "contrib/libdevel"),
            // A udeb is still a Binary package.
            libdevel("usr/bin/foo", elf()).udeb(),
        ];
        BinaryPackageTester::new(DevelopmentPackageShipsElfBinaryInPath, pass, fail)
            .test_and_snapshot();
    }
}
