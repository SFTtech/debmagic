use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::{emit, is_file, visit};

pub struct WindowsThumbnailDatabaseInPackage;

declare_rule! {
    /// A file named `Thumbs.db` or `Thumbs.db.gz`.
    ///
    /// The match is case-insensitive and must contain a slash, so a root
    /// `Thumbs.db` is not flagged. Regular files and hardlinks only.
    /// Udebs are checked too. Extra is empty.
    WindowsThumbnailDatabaseInPackage,
    code = "LN0411",
    tag = "windows-thumbnail-database-in-package",
    default_selected = true,
    experimental = false,
}

static THUMBS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/Thumbs\.db(?:\.gz)?$").expect("regex"));

impl BinaryPackageRule for WindowsThumbnailDatabaseInPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<String> = visit(files)
            .into_iter()
            .filter_map(|(entry, name)| (is_file(entry) && THUMBS.is_match(&name)).then_some(name))
            .collect();
        for path in hits {
            emit(ctx, path, Severity::Warning);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), file)])
    }

    #[test]
    fn test() {
        let store = TestFile::new("x");
        let pass = vec![
            file("Thumbs.db", store.clone()),
            file("usr/share/doc/pkg/thumbs.db.bak", store.clone()),
            file("usr/share/doc/pkg/Thumbs.db.gz.gz", store.clone()),
            file("usr/share/doc/pkg/Thumbs.db", TestFile::directory()),
            file(
                "usr/share/doc/pkg/Thumbs.db",
                TestFile::symlink("elsewhere"),
            ),
        ];
        let fail = vec![
            file("usr/share/doc/filenames/Thumbs.db", store.clone()),
            file("usr/share/pkg/thumbs.DB.gz", store),
            file("usr/share/pkg/Thumbs.db", TestFile::new("x").hardlink()).udeb(),
        ];
        BinaryPackageTester::new(WindowsThumbnailDatabaseInPackage, pass, fail).test_and_snapshot();
    }
}
