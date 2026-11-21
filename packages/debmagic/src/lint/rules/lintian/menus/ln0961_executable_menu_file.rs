use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::BinaryPackageRule;

pub struct ExecutableMenuFile;

declare_rule! {
    /// A menu file under `usr/lib/menu/` or `usr/share/menu/` has an execute
    /// bit.
    ///
    /// Regular files and hardlinks are checked; symlinks are not. The
    /// `README` in those directories is checked too. Extra is the permission
    /// bits in four-digit octal; the path is the Location. Udebs are checked
    /// too.
    ExecutableMenuFile,
    code = "LN0961",
    tag = "executable-menu-file",
    default_selected = true,
    experimental = false,
}

static MENU_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/(lib|share)/menu/\S").expect("valid regex"));

impl BinaryPackageRule for ExecutableMenuFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, String)> = files
            .entries()
            .iter()
            .filter(|entry| is_menu_file(entry))
            .map(|entry| (entry.path().to_path_buf(), format!("{:04o}", entry.mode())))
            .collect();
        for (path, message) in hits {
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn is_menu_file(entry: &Entry) -> bool {
    entry.is_executable_file()
        && entry
            .path()
            .to_str()
            .is_some_and(|path| MENU_FILE.is_match(path))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("usr/share/menu/test-menu", TestFile::new("x").mode(0o644)),
            // Only the menu directories count.
            data("usr/bin/test-menu", TestFile::new("x").mode(0o755)),
            data("usr/share/menus/test-menu", TestFile::new("x").mode(0o755)),
            // Directories and symlinks are not files.
            data("usr/share/menu", TestFile::directory().mode(0o755)),
            data(
                "usr/share/menu/link",
                TestFile::symlink("test-menu").mode(0o777),
            ),
        ];
        let fail = vec![
            data("usr/share/menu/test-menu", TestFile::new("x").mode(0o755)),
            data("usr/lib/menu/test-menu", TestFile::new("x").mode(0o700)),
            // The README is not exempt here.
            data("usr/share/menu/README", TestFile::new("x").mode(0o755)),
            data("usr/share/menu/test-menu", TestFile::new("x").mode(0o755)).udeb(),
        ];
        BinaryPackageTester::new(ExecutableMenuFile, pass, fail).test_and_snapshot();
    }
}
