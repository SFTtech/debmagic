use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::rule::SourceRule;

pub struct SourceContainsPrebuiltWindowsBinary;

declare_rule! {
    /// The Source tree or Source package contains a prebuilt Microsoft Windows binary.
    ///
    /// Regular files and hardlinks whose File type is PE32, PE64, or DOS MZ,
    /// or whose path ends in `.com`, are reported. Walks the packaging File
    /// tree (Source tree, or Source package Patched files). Quilt `.pc/` is
    /// filtered out; `debian/missing-sources/` is skipped. Extra is the
    /// Entry path. COM is a path heuristic, not File type.
    SourceContainsPrebuiltWindowsBinary,
    code = "LN0004",
    tag = "source-contains-prebuilt-windows-binary",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SourceContainsPrebuiltWindowsBinary {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        for path in windows_binary_paths(files) {
            let extra = path.display().to_string();
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn windows_binary_paths(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| {
            let path = entry.path();
            !path.starts_with(".pc")
                && !path.starts_with("debian/missing-sources")
                && is_windows_binary(entry)
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

fn is_windows_binary(entry: &Entry) -> bool {
    if entry
        .path()
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("com"))
    {
        return true;
    }
    entry.file_type().is_some_and(FileType::is_windows_binary)
}

#[cfg(test)]
mod tests {
    use crate::lint::file_tree::EntryKind;
    use crate::lint::file_type::FileType;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("README", TestFile::new("hello")),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    ".pc/foo.exe",
                    TestFile::new("ignored").file_type(FileType::Pe32),
                ),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "debian/missing-sources/foo.exe",
                    TestFile::new("sourced").file_type(FileType::Pe32),
                ),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "bin/app.exe",
                    TestFile::new("link")
                        .kind(EntryKind::Symlink)
                        .file_type(FileType::Pe32),
                ),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("bin/app.exe", TestFile::new("pe").file_type(FileType::Pe32)),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("lib/foo.dll", TestFile::new("pe").file_type(FileType::Pe64)),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "fake-win32-bin.exe",
                    TestFile::new("mz").file_type(FileType::DosMz),
                ),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("tools/legacy.com", TestFile::new("com")),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "shared/payload.exe",
                    TestFile::new("pe")
                        .kind(EntryKind::Hardlink)
                        .file_type(FileType::Pe32),
                ),
            ],
        ];
        SourceTreeTester::new(SourceContainsPrebuiltWindowsBinary, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pe = TestFile::new("pe").file_type(FileType::Pe32);
        let pass = vec![
            SourcePackageCase::from(vec![("README", TestFile::new("hello"))]),
            SourcePackageCase::from(vec![(
                ".pc/foo.exe",
                TestFile::new("ignored").file_type(FileType::Pe32),
            )]),
            SourcePackageCase::from(vec![(
                "debian/missing-sources/foo.exe",
                TestFile::new("sourced").file_type(FileType::Pe32),
            )])
            .orig(vec![(
                "bin/app.exe",
                TestFile::new("pe").file_type(FileType::Pe32),
            )]),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![("bin/app.exe", pe.clone())]),
            SourcePackageCase::from(vec![(
                "lib/foo.dll",
                TestFile::new("pe").file_type(FileType::Pe64),
            )]),
        ];
        SourcePackageTester::new(SourceContainsPrebuiltWindowsBinary, pass, fail)
            .test_and_snapshot();
    }
}
