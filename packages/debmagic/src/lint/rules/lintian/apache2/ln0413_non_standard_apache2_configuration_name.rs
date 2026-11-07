use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NonStandardApache2ConfigurationName;

declare_rule! {
    /// An Apache configuration file is not named `<package>.conf`.
    ///
    /// Files (not directories) in `etc/apache2/conf-available`,
    /// `site-available`, and `conf.d` are web-application configs. A name other
    /// than `<package>.conf`, or one that starts with `local-`, is reported.
    /// Packages named `apache2`, `apache2.2`, or `apache2-<word>` are skipped.
    /// Extra is `<file> != <package>.conf`. The path is the Location. Udebs are
    /// checked too.
    NonStandardApache2ConfigurationName,
    code = "LN0413",
    tag = "non-standard-apache2-configuration-name",
    default_selected = true,
    experimental = false,
}

static APACHE_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^apache2(?:\.2)?(?:-\w+)?$").expect("valid regex"));

impl BinaryPackageRule for NonStandardApache2ConfigurationName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        if APACHE_PACKAGE.is_match(&package) {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            if matches!(entry.kind(), EntryKind::Directory) {
                continue;
            }
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            let Some(webapp) = webapp_name(path) else {
                continue;
            };
            let expected = format!("{package}.conf");
            if webapp != expected || webapp.starts_with("local-") {
                hits.push((PathBuf::from(path), format!("{webapp} != {expected}")));
            }
        }
        hits.sort_by(|left, right| left.0.cmp(&right.0));
        for (path, message) in hits {
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn webapp_name(path: &str) -> Option<&str> {
    path.strip_prefix("etc/apache2/conf-available/")
        .or_else(|| path.strip_prefix("etc/apache2/site-available/"))
        .or_else(|| path.strip_prefix("etc/apache2/conf.d/"))
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn conf(package: &str, path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("# site\n"))])
            .package(package)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            conf("phpmyfoo", "etc/apache2/conf-available/phpmyfoo.conf"),
            conf("apache2", "etc/apache2/conf-available/other.conf"),
            conf("apache2-bin", "etc/apache2/conf.d/other.conf"),
            conf("phpmyfoo", "etc/apache2/mods-available/phpmyfoo.load"),
            BinaryPackageCase::from(vec![(
                "etc/apache2/conf-available".to_string(),
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            conf("phpmyfoo2", "etc/apache2/conf-available/phpmyfoo2"),
            conf(
                "phpmyfoo2",
                "etc/apache2/conf-available/local-phpmyfoo2.conf",
            ),
            conf("phpmyfoo3", "etc/apache2/conf.d/phpmyfoo.conf"),
            conf("site", "etc/apache2/site-available/local-site.conf"),
        ];
        BinaryPackageTester::new(NonStandardApache2ConfigurationName, pass, fail)
            .test_and_snapshot();
    }
}
