use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

/// Children of `usr/share/doc/<pkg>` whose basename is `NEWS.Debian` with an
/// optional `.gz`, case-insensitively, that do not end in lowercase `.gz`.
/// Empty when the doc directory is itself a symlink.
fn uncompressed_debian_news_paths(files: &FileTree, package: &str) -> Vec<PathBuf> {
    let doc = format!("usr/share/doc/{package}");
    if files
        .get(&doc)
        .is_some_and(|entry| entry.kind() == EntryKind::Symlink)
    {
        return Vec::new();
    }
    let dir = PathBuf::from(doc);
    files
        .entries()
        .iter()
        .filter_map(|entry| {
            if entry.path().parent() != Some(dir.as_path()) {
                return None;
            }
            if !matches!(
                entry.kind(),
                EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
            ) {
                return None;
            }
            let name = entry.path().file_name()?.to_str()?;
            let folded = name.to_ascii_lowercase();
            let news = folded == "news.debian" || folded == "news.debian.gz";
            (news && !name.ends_with(".gz")).then(|| entry.path().to_path_buf())
        })
        .collect()
}

pub struct DebianNewsFileNotCompressed;

declare_rule! {
    /// A Debian news file is installed without gzip compression.
    ///
    /// Looks at the files and symlinks directly in `usr/share/doc/<pkg>`
    /// (skipped when that path is itself a symlink) whose basename is
    /// `NEWS.Debian` with an optional `.gz`, ignoring letter case, and flags
    /// those not ending in lowercase `.gz`. Extra is empty. The Location is
    /// the file. Udebs are not checked.
    DebianNewsFileNotCompressed,
    code = "LN0660",
    tag = "debian-news-file-not-compressed",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebianNewsFileNotCompressed {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths = uncompressed_debian_news_paths(files, &package);
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn packaged(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(name.to_string(), TestFile::new("news\n"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            packaged("usr/share/doc/example/NEWS.Debian.gz"),
            packaged("usr/share/doc/example/news.debian.gz"),
            packaged("usr/share/doc/example/NEWS"),
            packaged("usr/share/doc/example/subdir/NEWS.Debian"),
            packaged("usr/share/doc/other/NEWS.Debian"),
            packaged("usr/share/doc/example/NEWS.Debian").udeb(),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/example", TestFile::symlink("other")),
                ("usr/share/doc/other/NEWS.Debian", TestFile::new("news\n")),
            ]),
        ];
        let fail = vec![
            packaged("usr/share/doc/example/NEWS.Debian"),
            packaged("usr/share/doc/example/news.debian"),
            packaged("usr/share/doc/example/NEWS.Debian.GZ"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/NEWS.Debian",
                TestFile::symlink("../other/NEWS.Debian"),
            )]),
        ];
        BinaryPackageTester::new(DebianNewsFileNotCompressed, pass, fail).test_and_snapshot();
    }
}
