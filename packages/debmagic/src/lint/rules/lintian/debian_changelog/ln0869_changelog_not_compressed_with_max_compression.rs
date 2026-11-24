use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct ChangelogNotCompressedWithMaxCompression;

declare_rule! {
    /// A compressed changelog in `usr/share/doc/<pkg>` was not made with `gzip -9`.
    ///
    /// Looks at each file or symlink directly under `usr/share/doc/<pkg>` whose
    /// name is `changelog`, optionally `.html` or `.Debian`, then `.gz`. A
    /// symlink is followed inside the package. The Rule fires when the
    /// resolved file is not gzip data with the maximum-compression header flag
    /// (XFL = 2, which is what `file(1)` reports as `max compression`). A
    /// dangling symlink is skipped, and so is the whole package when
    /// `usr/share/doc/<pkg>` is itself a symlink. Extra is empty; the Location is
    /// the changelog path. Udebs are not checked.
    ChangelogNotCompressedWithMaxCompression,
    code = "LN0869",
    tag = "changelog-not-compressed-with-max-compression",
    default_selected = true,
    experimental = false,
}

static CHANGELOG_GZ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^changelog(?:\.html|\.Debian)?\.gz$").expect("valid regex"));

const MAX_COMPRESSION_XFL: u8 = 2;

impl BinaryPackageRule for ChangelogNotCompressedWithMaxCompression {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let dir = PathBuf::from(format!("usr/share/doc/{}", ctx.package_name()));
        let offenders = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            if files
                .get(&dir)
                .is_some_and(|entry| entry.kind() == EntryKind::Symlink)
            {
                return;
            }
            changelogs_not_max_compressed(files, &dir)
        };
        for path in offenders {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn changelogs_not_max_compressed(files: &FileTree, dir: &Path) -> Vec<PathBuf> {
    files
        .entries()
        .iter()
        .filter(|entry| entry.path().parent() == Some(dir))
        .filter(|entry| {
            matches!(
                entry.kind(),
                EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
            )
        })
        .filter(|entry| {
            entry
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| CHANGELOG_GZ.is_match(name))
        })
        .filter_map(|entry| {
            let resolved = resolve(files, entry)?;
            if is_max_compressed(files, resolved) {
                None
            } else {
                Some(entry.path().to_path_buf())
            }
        })
        .collect()
}

/// Follows symlinks inside the tree, up to a fixed depth.
fn resolve<'a>(files: &'a FileTree, entry: &'a Entry) -> Option<&'a Entry> {
    let mut current = entry;
    for _ in 0..8 {
        if current.kind() != EntryKind::Symlink {
            return Some(current);
        }
        let target = current.link_target()?;
        if target.is_absolute() {
            return None;
        }
        let joined = current.path().parent()?.join(target);
        current = files.get(normalize(&joined)?)?;
    }
    None
}

fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

/// The gzip header (RFC 1952) of a regular file says maximum compression.
fn is_max_compressed(files: &FileTree, entry: &Entry) -> bool {
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return false;
    }
    let Ok(bytes) = files.read(entry) else {
        return false;
    };
    bytes.len() >= 10 && bytes[0] == 0x1f && bytes[1] == 0x8b && bytes[8] == MAX_COMPRESSION_XFL
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn gzip(text: &str, level: flate2::Compression) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), level);
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn file(name: &str, contents: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(name.to_string(), contents)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1"), flate2::Compression::best()),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry("1.0"), flate2::Compression::best()),
            ),
            // Not a changelog name, so not checked.
            file(
                "usr/share/doc/example/NEWS.Debian.gz",
                gzip(&entry("1.0-1"), flate2::Compression::default()),
            ),
            // Not compressed, so not this Rule's concern.
            file(
                "usr/share/doc/example/changelog.Debian",
                TestFile::new(entry("1.0-1")),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry("1.0-1"), flate2::Compression::default()),
            )
            .udeb(),
        ];
        let fail = vec![
            file(
                "usr/share/doc/example/changelog.Debian.gz",
                gzip(&entry("1.0-1"), flate2::Compression::default()),
            ),
            file(
                "usr/share/doc/example/changelog.gz",
                gzip(&entry("1.0-1"), flate2::Compression::fast()),
            ),
            file(
                "usr/share/doc/example/changelog.html.gz",
                TestFile::new("not gzip at all"),
            ),
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc/example/changelog.Debian.gz".to_string(),
                    gzip(&entry("1.0-1"), flate2::Compression::default()),
                ),
                (
                    "usr/share/doc/example/changelog.gz".to_string(),
                    TestFile::symlink("changelog.Debian.gz"),
                ),
            ]),
            // Dangling symlink is skipped; this one fires only for the valid name.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc/example/changelog.gz".to_string(),
                    TestFile::symlink("missing.gz"),
                ),
                (
                    "usr/share/doc/example/changelog.Debian.gz".to_string(),
                    gzip(&entry("1.0-1"), flate2::Compression::default()),
                ),
            ]),
        ];
        BinaryPackageTester::new(ChangelogNotCompressedWithMaxCompression, pass, fail)
            .test_and_snapshot();
    }
}
