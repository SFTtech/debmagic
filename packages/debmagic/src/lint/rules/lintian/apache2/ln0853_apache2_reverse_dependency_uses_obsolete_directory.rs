use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct Apache2ReverseDependencyUsesObsoleteDirectory;

declare_rule! {
    /// An Apache reverse dependency ships a file in the obsolete
    /// `etc/apache2/conf.d/` directory.
    ///
    /// Reported: each non-directory under `etc/apache2/conf.d/`, whatever its
    /// name. Extra is empty, so the Location (the file) is part of the message.
    /// The web server packages are exempt.
    Apache2ReverseDependencyUsesObsoleteDirectory,
    code = "LN0853",
    tag = "apache2-reverse-dependency-uses-obsolete-directory",
    default_selected = true,
    experimental = false,
}

static CONF_D: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/apache2/conf\.d/.*$").expect("valid regex"));

impl BinaryPackageRule for Apache2ReverseDependencyUsesObsoleteDirectory {
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
                        .is_some_and(|path| CONF_D.is_match(path))
                })
                .map(|entry| entry.path().to_path_buf())
                .collect::<Vec<_>>()
        };
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
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            file("etc/apache2/conf-available/foo.conf"),
            BinaryPackageCase::from(vec![(
                "etc/apache2/conf.d".to_string(),
                TestFile::directory(),
            )]),
            file("etc/apache2/conf.d/foo.conf").package("apache2"),
        ];
        let fail = vec![
            file("etc/apache2/conf.d/phpmyfoo.conf"),
            file("etc/apache2/conf.d/foo"),
        ];
        BinaryPackageTester::new(Apache2ReverseDependencyUsesObsoleteDirectory, pass, fail)
            .test_and_snapshot();
    }
}
