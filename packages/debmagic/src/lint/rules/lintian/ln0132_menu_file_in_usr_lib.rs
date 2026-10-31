use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct MenuFileInUsrLib;

declare_rule! {
    /// A menu file is installed under `usr/lib/menu/`.
    ///
    /// `usr/lib/menu/README` is exempt. Only regular files and hardlinks.
    /// Extra is empty; the path is the Location. Udebs are checked too.
    MenuFileInUsrLib,
    code = "LN0132",
    tag = "menu-file-in-usr-lib",
    default_selected = true,
    experimental = false,
}

static MENU: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/menu/\S").expect("valid regex"));

impl BinaryPackageRule for MenuFileInUsrLib {
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
                if !MENU.is_match(path)
                    || path.ends_with("/README")
                    || path == "usr/lib/menu/README"
                {
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

    fn file(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("x"))])
    }

    #[test]
    fn test() {
        let pass = vec![file("usr/share/menu/example"), file("usr/lib/menu/README")];
        let fail = vec![file("usr/lib/menu/binary"), file("usr/lib/menu/menu")];
        BinaryPackageTester::new(MenuFileInUsrLib, pass, fail).test_and_snapshot();
    }
}
