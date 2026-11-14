use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct FileInUsrLibSitePython;

declare_rule! {
    /// A path sits under `usr/lib/site-python/`.
    ///
    /// The directory itself is not enough; something must follow the slash.
    /// Directories are reported with a trailing slash. Extra is empty.
    /// Udebs are checked too.
    FileInUsrLibSitePython,
    code = "LN0130",
    tag = "file-in-usr-lib-site-python",
    default_selected = true,
    experimental = false,
}

static PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/site-python/\S").expect("valid regex"));

impl BinaryPackageRule for FileInUsrLibSitePython {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter_map(|entry| {
                let raw = entry.path().to_str()?;
                let name = if entry.kind() == EntryKind::Directory {
                    format!("{raw}/")
                } else {
                    raw.to_string()
                };
                PATH.is_match(&name).then_some(PathBuf::from(name))
            })
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
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

    #[test]
    fn test() {
        let pass = vec![BinaryPackageCase::from(vec![(
            "usr/lib/site-python".to_string(),
            TestFile::directory(),
        )])];
        let fail = vec![BinaryPackageCase::from(vec![
            ("usr/lib/site-python/foo".to_string(), TestFile::directory()),
            (
                "usr/lib/site-python/foo/bar".to_string(),
                TestFile::new("x"),
            ),
        ])];
        BinaryPackageTester::new(FileInUsrLibSitePython, pass, fail).test_and_snapshot();
    }
}
