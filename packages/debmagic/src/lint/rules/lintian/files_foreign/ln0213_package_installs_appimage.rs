use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::{emit, is_file, visit};

pub struct PackageInstallsAppimage;

declare_rule! {
    /// An AppImage, by name or by magic.
    ///
    /// A regular file or hardlink whose name ends in `.AppImage` (any case).
    /// Separately, the first 11 bytes end in `41 49 02` (shorter files do
    /// not). A path that is both emits two hints, the name first. Udebs are
    /// checked too. Extra is empty.
    PackageInstallsAppimage,
    code = "LN0213",
    tag = "package-installs-appimage",
    default_selected = true,
    experimental = false,
}

static APPIMAGE_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.AppImage$").expect("valid regex"));

const AI_MAGIC: [u8; 3] = [0x41, 0x49, 0x02];
const AI_MAGIC_LEN: usize = 11;

impl BinaryPackageRule for PackageInstallsAppimage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for (entry, name) in visit(files) {
            if !is_file(entry) {
                continue;
            }
            if APPIMAGE_NAME.is_match(&name) {
                hits.push(name.clone());
            }
            if is_appimage_magic(files, entry) {
                hits.push(name);
            }
        }
        for path in hits {
            emit(ctx, path, Severity::Error);
        }
    }
}

/// `magic(11)` then `/AI\x02$/`. Files shorter than 11 bytes are empty magic.
fn is_appimage_magic(files: &FileTree, entry: &Entry) -> bool {
    let Ok(bytes) = files.read(entry) else {
        return false;
    };
    bytes.len() >= AI_MAGIC_LEN && bytes[8..AI_MAGIC_LEN] == AI_MAGIC
}

#[cfg(test)]
mod tests {
    use crate::lint::file_type::FileType;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn magic() -> TestFile {
        TestFile::from_bytes(vec![
            0x7f_u8, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x01, 0x00, 0x41, 0x49, 0x02,
        ])
    }

    fn file(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), file)])
    }

    #[test]
    fn test() {
        let elf_without_magic =
            TestFile::from_bytes(vec![0x7f_u8, 0x45, 0x4c, 0x46, 0, 0, 0, 0, 0, 0, 0]).file_type(
                FileType::Elf {
                    has_static_symbol_table: false,
                    executable: true,
                },
            );
        let pass = vec![
            file("usr/bin/dodgy-software.AppImage.bak", TestFile::new("x")),
            file("usr/bin/dodgy-software.AppImages", TestFile::new("x")),
            file("usr/bin/tool.AppImage", TestFile::directory()),
            file("usr/bin/tool.AppImage", TestFile::symlink("elsewhere")),
            file(
                "usr/bin/short",
                TestFile::from_bytes(vec![0_u8, 0, 0, 0, 0, 0, 0, 0x41, 0x49, 0x02]),
            ),
            file(
                "usr/bin/offset",
                TestFile::from_bytes(vec![0x41_u8, 0x49, 0x02, 0, 0, 0, 0, 0, 0, 0, 0]),
            ),
            file("usr/bin/elf-tool", elf_without_magic),
        ];
        let fail = vec![
            file(
                "usr/share/appimage-file/dodgy-software.AppImage",
                TestFile::new("# not magic"),
            ),
            file("usr/bin/app.appimage", TestFile::new("x")),
            file("usr/bin/dodgy-software", magic()),
            file("usr/bin/tool.AppImage", magic()),
            file("usr/bin/linked.AppImage", TestFile::new("x").hardlink()).udeb(),
        ];
        BinaryPackageTester::new(PackageInstallsAppimage, pass, fail).test_and_snapshot();
    }
}
