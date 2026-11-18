use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct ObsoleteCommentsStyleInPhpIni;

declare_rule! {
    /// A PHP module configuration file under `etc/php/*/mods-available/` has a
    /// line that starts with `#` as a comment.
    ///
    /// Regular files and hardlinks whose path matches
    /// `etc/php/.*/mods-available/.+\.ini` are read. A line starting with `#`,
    /// after optional whitespace, is reported. PHP has warned about that
    /// comment style since 5.3; `;` is the separator to use. Reported once per
    /// file at the path with an empty extra. Udebs are checked too.
    ObsoleteCommentsStyleInPhpIni,
    code = "LN0820",
    tag = "obsolete-comments-style-in-php-ini",
    default_selected = true,
    experimental = false,
}

static MODS_AVAILABLE_INI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/php/.*/mods-available/.+\.ini$").expect("valid regex"));

static HASH_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*#").expect("valid regex"));

impl BinaryPackageRule for ObsoleteCommentsStyleInPhpIni {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .path(&MODS_AVAILABLE_INI)
            .into_iter()
            .filter(|entry| {
                files
                    .read(entry)
                    .is_ok_and(|bytes| HASH_COMMENT.is_match(&String::from_utf8_lossy(&bytes)))
            })
            .map(|entry| entry.path().to_path_buf())
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

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            tree(vec![(
                "etc/php/7.0/mods-available/foo.ini",
                TestFile::new("; comment\nextension=foo.so\n"),
            )]),
            // A `#` that does not start a line is not a comment.
            tree(vec![(
                "etc/php/7.0/mods-available/foo.ini",
                TestFile::new("extension=foo.so # x\n"),
            )]),
            // Only `mods-available` `.ini` files are checked.
            tree(vec![(
                "etc/php/7.0/apache2/php.ini",
                TestFile::new("# comment\n"),
            )]),
            tree(vec![(
                "etc/php/7.0/mods-available/foo.txt",
                TestFile::new("# comment\n"),
            )]),
            tree(vec![(
                "etc/php/7.0/mods-available/foo.ini",
                TestFile::symlink("bar.ini"),
            )]),
            tree(vec![(
                "etc/php/7.0/mods-available/foo.ini",
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            tree(vec![(
                "etc/php/7.0/mods-available/foo.ini",
                TestFile::new("# comment\n"),
            )]),
            // Leading whitespace is allowed before the `#`.
            tree(vec![(
                "etc/php/7.0/mods-available/foo.ini",
                TestFile::new("extension=foo.so\n   # comment\n"),
            )]),
            tree(vec![(
                "etc/php/7.0/mods-available/sub/foo.ini",
                TestFile::new("#\n"),
            )]),
            tree(vec![(
                "etc/php/7.0/mods-available/foo.ini",
                TestFile::new("# a\n").hardlink(),
            )]),
            tree(vec![(
                "etc/php/7.0/mods-available/foo.ini",
                TestFile::new("# comment\n"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(ObsoleteCommentsStyleInPhpIni, pass, fail).test_and_snapshot();
    }
}
