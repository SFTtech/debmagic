use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct BadMenuFileName;

declare_rule! {
    /// A menu file is named `menu`.
    ///
    /// `usr/lib/menu/menu` and `usr/share/menu/menu`, except in the `menu`
    /// package. Only regular files and hardlinks. Extra is empty; the path is
    /// the Location. Udebs are checked too.
    BadMenuFileName,
    code = "LN0133",
    tag = "bad-menu-file-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BadMenuFileName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name() == "menu" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                matches!(path, "usr/lib/menu/menu" | "usr/share/menu/menu")
                    .then_some(PathBuf::from(path))
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

    fn file(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("x"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            file("usr/share/menu/example"),
            file("usr/share/menu/menu").package("menu"),
        ];
        let fail = vec![file("usr/share/menu/menu"), file("usr/lib/menu/menu")];
        BinaryPackageTester::new(BadMenuFileName, pass, fail).test_and_snapshot();
    }
}
