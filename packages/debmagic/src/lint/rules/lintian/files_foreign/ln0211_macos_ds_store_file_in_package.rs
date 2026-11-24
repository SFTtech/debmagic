use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::{emit, is_file, visit};

pub struct MacosDsStoreFileInPackage;

declare_rule! {
    /// A file named `.DS_Store` or `.DS_Store.gz`.
    ///
    /// The name is case-sensitive and must contain a slash, so a root
    /// `.DS_Store` is not flagged. Regular files and hardlinks only.
    /// Udebs are checked too. Extra is empty.
    MacosDsStoreFileInPackage,
    code = "LN0211",
    tag = "macos-ds-store-file-in-package",
    default_selected = true,
    experimental = false,
}

static DS_STORE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/\.DS_Store(?:\.gz)?$").expect("valid regex"));

impl BinaryPackageRule for MacosDsStoreFileInPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<String> = visit(files)
            .into_iter()
            .filter_map(|(entry, name)| {
                (is_file(entry) && DS_STORE.is_match(&name)).then_some(name)
            })
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
            file(".DS_Store", store.clone()),
            file("usr/share/doc/pkg/.ds_store", store.clone()),
            file("usr/share/doc/pkg/.DS_Store.bak", store.clone()),
            file("usr/share/doc/pkg/.DS_Store.gz.gz", store.clone()),
            file("usr/share/doc/pkg/.DS_Store", TestFile::directory()),
            file(
                "usr/share/doc/pkg/.DS_Store",
                TestFile::symlink("elsewhere"),
            ),
        ];
        let fail = vec![
            file("usr/share/doc/filenames/.DS_Store", store.clone()),
            file("usr/share/pkg/.DS_Store.gz", store),
            file("usr/share/pkg/.DS_Store", TestFile::new("x").hardlink()).udeb(),
        ];
        BinaryPackageTester::new(MacosDsStoreFileInPackage, pass, fail).test_and_snapshot();
    }
}
