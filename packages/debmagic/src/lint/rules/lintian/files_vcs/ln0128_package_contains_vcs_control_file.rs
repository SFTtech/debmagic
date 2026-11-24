use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsVcsControlFile;

declare_rule! {
    /// The package ships a version-control metadata file.
    ///
    /// Matches lintian's `files/vcs-control-files` patterns, including a
    /// compressed extension (`gz`, `bz2`, …). Files under
    /// `usr/share/cargo/registry/` are exempt. Directories are the other Tag.
    /// Extra is empty; the path is the Location. Udebs are checked too.
    PackageContainsVcsControlFile,
    code = "LN0128",
    tag = "package-contains-vcs-control-file",
    default_selected = true,
    experimental = false,
}

static PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    let extensions = format!(
        "(?:{})",
        crate::lint::data::lines(include_str!(
            "../../../data/files/compressed-file-extensions"
        ))
        .collect::<Vec<_>>()
        .join("|")
    );
    let patterns = crate::lint::data::lines(include_str!("../../../data/files/vcs-control-files"))
        .map(|line| {
            format!(
                "(?:{})",
                line.replace("${COMPRESS_EXT}", &extensions)
                    .replace("\\Z", "\\z")
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&patterns).expect("valid regex")
});

impl BinaryPackageRule for PackageContainsVcsControlFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                if path.starts_with("usr/share/cargo/registry/") || !PATTERN.is_match(path) {
                    return None;
                }
                Some(PathBuf::from(path))
            })
            .collect();
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

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| ((*path).to_string(), TestFile::new("x")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            files(&["usr/share/doc/example/README"]),
            files(&["usr/share/cargo/registry/foo/.gitignore"]),
            files(&[".gitignore"]),
        ];
        let fail = vec![
            files(&["usr/share/foo/.gitignore", "usr/share/foo/.gitignore.gz"]),
            files(&["files/.cvsignore", "foo/.hgtags", "foo/.hg_archival.txt"]),
            files(&[
                "foo/.arch-inventory",
                "foo/.gitmodules",
                "foo/.gitattributes",
            ]),
        ];
        BinaryPackageTester::new(PackageContainsVcsControlFile, pass, fail).test_and_snapshot();
    }
}
