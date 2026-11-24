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
    /// Regular files and hardlinks whose File type is PE32, PE64, or DOS MZ
    /// are reported, as lintian's `(?:PE(?:32|64)|MS-DOS)` file type match.
    /// Walks the packaging File tree (Source tree, or Source package Patched
    /// files). Quilt `.pc/` is filtered out; `debian/missing-sources/` is
    /// skipped. The Entry path is the Location and the extra is empty, as
    /// lintian's `pointed_hint(tag, $item->pointer)` prints `tag [path]`.
    /// No path heuristic: a `.com` text file (VMS command procedure, domain
    /// welcomelist) is not a binary.
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
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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
    entry.file_type().is_some_and(FileType::is_windows_binary)
}

#[cfg(test)]
mod tests {
    use crate::lint::file_type::FileType;
    use crate::lint::tester::{
        SourcePackageCase, SourcePackageTester, SourceTreeTester, TestCase, TestFile,
    };

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
                ("bin/app.exe", TestFile::symlink("app.exe")),
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
                (
                    "tools/legacy.com",
                    TestFile::new("com").file_type(FileType::DosMz),
                ),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "shared/payload.exe",
                    TestFile::new("pe").file_type(FileType::Pe32).hardlink(),
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

    /// Lintian's `visit_patched_files` reports only on the File type regex
    /// (`PE(32|64)` or `MS-DOS|COM executable`), never on the path, so a
    /// text file with a Windows-looking extension is not reported.
    #[test]
    fn extension_alone_is_not_reported() {
        let pass = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("tools/build.com", TestFile::new("$ set verify\n")),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("data/hosts/example.com", TestFile::new("example.com\n")),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("tools/setup.exe", TestFile::new("not a program\n")),
            ],
        ];
        let fail = vec![];
        SourceTreeTester::new(SourceContainsPrebuiltWindowsBinary, pass, fail).test();
    }

    /// Lintian's hint is `tag [path]`, so the Entry path is the pointer: the
    /// Diagnostic has it as its Location and an empty message.
    #[test]
    fn hint_points_at_the_entry_path() {
        let tester = SourceTreeTester::new(
            SourceContainsPrebuiltWindowsBinary,
            Vec::<TestCase>::new(),
            Vec::<TestCase>::new(),
        );
        let diagnostics = tester.diagnostics(vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("bin/app", TestFile::new("pe").file_type(FileType::Pe32)),
        ]);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.is_empty());
        assert_eq!(diagnostics[0].location.path, PathBuf::from("bin/app"));
    }

    /// The File type decides regardless of the name: a PE or DOS MZ file
    /// without a Windows extension is reported.
    #[test]
    fn file_type_is_reported_without_extension() {
        let pass = vec![];
        let fail = vec![vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("bin/app", TestFile::new("pe").file_type(FileType::Pe32)),
            ("bin/tool", TestFile::new("mz").file_type(FileType::DosMz)),
        ]];
        SourceTreeTester::new(SourceContainsPrebuiltWindowsBinary, pass, fail).test();
    }
}
