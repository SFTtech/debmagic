use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct IconSizeAndDirectoryNameMismatch;

declare_rule! {
    /// A PNG icon's pixel size does not match its `NxN` directory.
    ///
    /// Paths containing `/icons/<theme>/<W>x<H>/` (not `animations/`) and ending
    /// in `.png`. Symlinks are followed inside the package. The PNG `IHDR`
    /// width and height are the file size. A difference of more than 2 pixels
    /// in either axis is reported. Extra is `<width>x<height>` of the file.
    /// The path is the Location. Udebs are checked too.
    IconSizeAndDirectoryNameMismatch,
    code = "LN0421",
    tag = "icon-size-and-directory-name-mismatch",
    default_selected = true,
    experimental = false,
}

static ICON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/icons/[^/]+/(\d+)x(\d+)/(.*)\.png$").expect("valid regex"));

impl BinaryPackageRule for IconSizeAndDirectoryNameMismatch {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            let Some(found) = ICON.captures(path) else {
                continue;
            };
            if found[3].starts_with("animations/") {
                continue;
            }
            let Ok(dir_width) = found[1].parse::<u32>() else {
                continue;
            };
            let Ok(dir_height) = found[2].parse::<u32>() else {
                continue;
            };
            let Some(resolved) = resolve_file(files, entry) else {
                continue;
            };
            let Ok(bytes) = files.read(resolved) else {
                continue;
            };
            let Some((width, height)) = png_size(&bytes) else {
                continue;
            };
            if dir_width.abs_diff(width) > 2 || dir_height.abs_diff(height) > 2 {
                hits.push((entry.path().to_path_buf(), format!("{width}x{height}")));
            }
        }
        hits.sort_by(|left, right| left.0.cmp(&right.0));
        for (path, message) in hits {
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn resolve_file<'a>(files: &'a FileTree, entry: &'a Entry) -> Option<&'a Entry> {
    let mut current = entry;
    for _ in 0..8 {
        if matches!(current.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            return Some(current);
        }
        let target = current.link_target()?;
        let next_path = if target.is_absolute() {
            PathBuf::from(target.to_str()?.trim_start_matches('/'))
        } else {
            lexical_join(current.path().parent().unwrap_or(Path::new("")), target)
        };
        current = files.get(&next_path)?;
    }
    None
}

fn lexical_join(base: &Path, target: &Path) -> PathBuf {
    let mut joined = base.to_path_buf();
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                joined.pop();
            }
            Component::Normal(part) => joined.push(part),
            Component::RootDir => joined = PathBuf::new(),
            Component::Prefix(_) => {}
        }
    }
    joined
}

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn png(width: u32, height: u32) -> TestFile {
        let mut bytes = Vec::from(&b"\x89PNG\r\n\x1a\n"[..]);
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 2, 0, 0, 0]);
        TestFile::from_bytes(bytes)
    }

    fn icon(path: &str, width: u32, height: u32) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), png(width, height))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            icon("usr/share/icons/hicolor/22x22/apps/ok.png", 22, 22),
            icon("usr/share/icons/hicolor/22x22/apps/close.png", 24, 20),
            icon("usr/share/icons/hicolor/22x22/animations/move.png", 16, 16),
            BinaryPackageCase::from(vec![(
                "usr/share/icons/hicolor/64x64/apps/text.png".to_string(),
                TestFile::new("not a png"),
            )]),
        ];
        let fail = vec![
            icon(
                "usr/share/icons/hicolor/22x22/apps/lintian-16x16.png",
                16,
                16,
            ),
            icon(
                "usr/share/apps/lintian/icons/hicolor/64x64/lintian-64x64.png",
                22,
                22,
            ),
            BinaryPackageCase::from(vec![
                (
                    "usr/share/apps/lintian/icons/hicolor/22x22/lintian-22x22.png".to_string(),
                    png(22, 22),
                ),
                (
                    "usr/share/apps/lintian/icons/hicolor/64x64/lintian-64x64.png".to_string(),
                    TestFile::symlink("../22x22/lintian-22x22.png"),
                ),
            ]),
        ];
        BinaryPackageTester::new(IconSizeAndDirectoryNameMismatch, pass, fail).test_and_snapshot();
    }
}
