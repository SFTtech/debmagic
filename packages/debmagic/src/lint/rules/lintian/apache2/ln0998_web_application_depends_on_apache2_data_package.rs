use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct WebApplicationDependsOnApache2DataPackage;

declare_rule! {
    /// A web application declares a relation with `apache2-bin`, `apache2-data` or
    /// `apache2-common`: "The package appears to be a web application but declares a
    /// package relation with apache2-bin, apache2-data or any of its transitional
    /// packages. However, web applications are rarely bound to a specific web server
    /// version. Thus, they should depend on apache2 only instead."
    ///
    /// Lintian's `apache2` Check: a web application is a non-directory file under
    /// `etc/apache2/conf-available/`, `etc/apache2/site-available/` or
    /// `etc/apache2/conf.d/`. The relations are `Pre-Depends`, `Depends` and
    /// `Recommends`, AND-ed. The first alternative naming one of the packages is
    /// reported, once per web application file, with the package and the file's
    /// name (relative to its directory) as the extra. The file is the Location.
    /// Packages that are the web server itself (`apache2`, `apache2.2-*`) are
    /// skipped. Differences: a non-UTF-8 web application path is skipped.
    WebApplicationDependsOnApache2DataPackage,
    code = "LN0998",
    tag = "web-application-depends-on-apache2-data-package",
    default_selected = true,
    experimental = false,
}

/// The web server packages that lintian's `apache2-(common|data|bin)` pattern names.
static DATA_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^apache2(?:\.2)?-(?:common|data|bin)$").expect("valid regex"));

/// The web server itself, which lintian's Check does not look at.
static WEB_SERVER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^apache2(:?\.2)?(?:-\w+)?$").expect("valid regex"));

/// `etc/apache2/(conf|site)-available/<name>` and `etc/apache2/conf.d/<name>`.
static WEB_APPLICATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^etc/apache2/(?:(?:conf|site)-available/|conf\.d/)(.*)$").expect("valid regex")
});

impl BinaryPackageRule for WebApplicationDependsOnApache2DataPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if WEB_SERVER.is_match(ctx.package_name()) {
            return;
        }
        let relation = ctx.installation_control().fields().relations_and(&[
            "Pre-Depends",
            "Depends",
            "Recommends",
        ]);
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, webapp) in web_applications(files) {
            let Some(package) = relation
                .alternatives()
                .map(|alternative| alternative.package())
                .find(|package| DATA_PACKAGE.is_match(package))
            else {
                continue;
            };
            ctx.diagnostic(
                Diagnostic::warn(format!("{package} {webapp}")).with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

/// Non-directory web application files and their names, in path order.
fn web_applications(files: &FileTree) -> Vec<(PathBuf, String)> {
    files
        .entries()
        .iter()
        .filter(|entry| !entry.is_dir())
        .filter_map(|entry| {
            let path = entry.path().to_str()?;
            let webapp = WEB_APPLICATION.captures(path)?.get(1)?.as_str().to_string();
            Some((entry.path().to_path_buf(), webapp))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(depends: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "etc/apache2/conf-available/phpmyfoo.conf",
            TestFile::new("# web application\n"),
        )])
        .package("phpmyfoo")
        .field("Depends", depends)
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Depends on the web server, not its data packages.
            package("apache2"),
            package("apache2 | httpd"),
            // Not a web application: nothing installed under the directories.
            BinaryPackageCase::from(vec![("etc/apache2/conf-available/", TestFile::directory())])
                .package("phpmyfoo")
                .field("Depends", "apache2-bin"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/phpmyfoo/README",
                TestFile::new("doc"),
            )])
            .package("phpmyfoo")
            .field("Depends", "apache2-bin"),
            // The web server itself.
            BinaryPackageCase::from(vec![(
                "etc/apache2/conf-available/apache2.conf",
                TestFile::new(""),
            )])
            .package("apache2")
            .field("Depends", "apache2-bin"),
            // Another apache2 package is not one of the listed packages.
            package("apache2-utils"),
        ];
        let fail = vec![
            package("apache2-bin"),
            package("apache2, apache2-data"),
            package("apache2-common"),
            package("apache2.2-bin"),
            // Recommends counts too.
            package("apache2").field("Recommends", "apache2-bin"),
            // conf.d is a web application directory as well.
            BinaryPackageCase::from(vec![(
                "etc/apache2/conf.d/phpmyfoo.conf",
                TestFile::new(""),
            )])
            .package("phpmyfoo")
            .field("Depends", "apache2-data"),
            // site-available counts too, and only the first match is reported.
            BinaryPackageCase::from(vec![
                ("etc/apache2/site-available/a.conf", TestFile::new("")),
                ("etc/apache2/site-available/b.conf", TestFile::new("")),
            ])
            .package("phpmyfoo")
            .field("Depends", "apache2-bin, apache2-data"),
        ];
        BinaryPackageTester::new(WebApplicationDependsOnApache2DataPackage, pass, fail)
            .test_and_snapshot();
    }
}
