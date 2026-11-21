use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct Apache2ReverseDependencyShipsFileInNotAllowedDirectory;

declare_rule! {
    /// An Apache reverse dependency ships a file in `etc/apache2/` enabled
    /// directories, which only `a2enconf`, `a2ensite` and `a2enmod` may write.
    ///
    /// Reported: each non-directory under `etc/apache2/conf-enabled/`,
    /// `etc/apache2/sites-enabled/` or `etc/apache2/mods-enabled/`. Lintian's
    /// pattern spells the site directory `sites-enabled`, so a file under
    /// `site-enabled/` is not reported. Extra is empty, so the Location (the
    /// file) is part of the message. The web server packages are exempt.
    Apache2ReverseDependencyShipsFileInNotAllowedDirectory,
    code = "LN0852",
    tag = "apache2-reverse-dependency-ships-file-in-not-allowed-directory",
    default_selected = true,
    experimental = false,
}

static ENABLED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^etc/apache2/(?:conf|sites|mods)-enabled/.*$").expect("valid regex")
});

impl BinaryPackageRule for Apache2ReverseDependencyShipsFileInNotAllowedDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if scan::is_web_server(ctx.package_name()) {
            return;
        }
        let paths = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            files
                .entries()
                .iter()
                .filter(|entry| entry.kind() != EntryKind::Directory)
                .filter(|entry| {
                    entry
                        .path()
                        .to_str()
                        .is_some_and(|path| ENABLED.is_match(path))
                })
                .map(|entry| entry.path().to_path_buf())
                .collect::<Vec<_>>()
        };
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
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            file("etc/apache2/conf-available/foo.conf"),
            file("etc/apache2/mods-available/foo.load"),
            file("etc/apache2/site-enabled/foo.conf"),
            BinaryPackageCase::from(vec![(
                "etc/apache2/mods-enabled".to_string(),
                TestFile::directory(),
            )]),
            file("etc/apache2/mods-enabled/foo.conf").package("apache2"),
        ];
        let fail = vec![
            file("etc/apache2/mods-enabled/foo.conf"),
            file("etc/apache2/conf-enabled/foo.conf"),
            file("etc/apache2/sites-enabled/000-default.conf"),
        ];
        BinaryPackageTester::new(
            Apache2ReverseDependencyShipsFileInNotAllowedDirectory,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
