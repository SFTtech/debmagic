use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::BinaryPackageRule;

pub struct ExecutableNotElfOrScript;

declare_rule! {
    /// An executable regular file is neither an ELF file nor a script.
    ///
    /// Walks regular Data files with any execute bit. Hardlinks, directories
    /// and symlinks are not walked. A file is fine when it starts
    /// with the ELF magic, or with `#!` and not the lincity magic `#!#!#!`.
    /// Paths under `usr/man/` and `usr/X11R6/man/`, and names ending in `.exe`
    /// or `.jar`, are left out. Extra is empty; the path is the Location.
    /// Udebs are checked too.
    ExecutableNotElfOrScript,
    code = "LN0710",
    tag = "executable-not-elf-or-script",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ExecutableNotElfOrScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| is_candidate(entry) && is_unexpected(files, entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// A regular file with an execute bit, outside the excluded paths.
fn is_candidate(entry: &Entry) -> bool {
    matches!(entry.kind(), EntryKind::RegularFile)
        && entry.is_executable_file()
        && !excluded(entry.path())
}

fn excluded(path: &Path) -> bool {
    let Some(name) = path.to_str() else {
        return false;
    };
    name.starts_with("usr/man/")
        || name.starts_with("usr/X11R6/man/")
        || name.ends_with(".exe")
        || name.ends_with(".jar")
}

/// Neither ELF nor script. Unreadable content is not reported.
fn is_unexpected(files: &FileTree, entry: &Entry) -> bool {
    if matches!(entry.file_type(), Some(FileType::Elf { .. })) {
        return false;
    }
    files
        .read(entry)
        .is_ok_and(|bytes| !is_elf(&bytes) && !Hashbang::is_script(&bytes))
}

fn is_elf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x7fELF")
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    fn exec(content: &str) -> TestFile {
        TestFile::new(content).mode(0o755)
    }

    fn elf() -> TestFile {
        TestFile::from_bytes(TestElf::executable().build()).mode(0o755)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("usr/bin/tool", exec("#!/bin/sh\necho hi\n")),
            data("usr/bin/tool", elf()),
            data("usr/bin/hash", exec("#!\n#!/bin/sh\n")),
            // Not executable: other Tags.
            data(
                "usr/share/doc/pkg/README",
                TestFile::new("not a script\n").mode(0o644),
            ),
            // Directories and symlinks are not files.
            data("usr/bin/dir", TestFile::directory().mode(0o755)),
            data("usr/bin/link", TestFile::symlink("tool").mode(0o777)),
            // Hardlinks are not walked.
            data("usr/bin/hard", exec("not a script\n").hardlink()),
            // Excluded paths and extensions.
            data("usr/man/man1/tool.1", exec("not a script\n")),
            data("usr/X11R6/man/man1/tool.1", exec("not a script\n")),
            data("usr/lib/dotnet/app.exe", exec("MZ not a script\n")),
            data("usr/share/java/app.jar", exec("PK not a script\n")),
        ];
        let fail = vec![
            data("usr/bin/empty", exec("")),
            data("usr/bin/text", exec("not a script\n")),
            data(
                "usr/share/applications/app.desktop",
                exec("[Desktop Entry]\n"),
            ),
            // Lincity magic is not a script.
            data("usr/bin/lincity", exec("#!#!#!\n")),
            // Only `#!` at the start counts.
            data("usr/bin/late", exec(" #!/bin/sh\n")),
            // Other executable formats are not ELF.
            data(
                "usr/bin/dos",
                TestFile::from_bytes(b"MZ\x90\x00").mode(0o755),
            ),
            // Setuid files are checked too.
            data("usr/bin/suid", exec("not a script\n").mode(0o4755)),
            data("bin/udeb-tool", exec("not a script\n")).udeb(),
        ];
        BinaryPackageTester::new(ExecutableNotElfOrScript, pass, fail).test_and_snapshot();
    }
}
