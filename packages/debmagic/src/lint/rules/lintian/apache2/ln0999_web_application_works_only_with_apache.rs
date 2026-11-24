use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct WebApplicationWorksOnlyWithApache;

declare_rule! {
    /// A web application depends on `apache2` without an alternative: "The package
    /// appears to be a web application but declares a dependency against apache2
    /// without any alternative. Most web applications should work with any decent
    /// web server, thus such a package should be satisfied if any web server
    /// providing the virtual httpd package is installed. This can be accomplished by
    /// declaring a package relation in the form apache2 | httpd."
    ///
    /// Lintian's `apache2` Check: a web application is a non-directory file under
    /// `etc/apache2/conf-available/`, `etc/apache2/site-available/` or
    /// `etc/apache2/conf.d/`. The relations are `Pre-Depends`, `Depends` and
    /// `Recommends`, AND-ed; the Rule reports when that relation implies `apache2`
    /// (an alternative such as `apache2 | httpd` does not). The web application's
    /// name relative to its directory is the extra; the file is the Location. The
    /// web server itself (`apache2`, `apache2.2-*`) is skipped. Differences: a
    /// non-UTF-8 web application path is skipped.
    WebApplicationWorksOnlyWithApache,
    code = "LN0999",
    tag = "web-application-works-only-with-apache",
    default_selected = true,
    experimental = false,
}

/// The web server itself, which lintian's Check does not look at.
static WEB_SERVER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^apache2(:?\.2)?(?:-\w+)?$").expect("valid regex"));

/// `etc/apache2/(conf|site)-available/<name>` and `etc/apache2/conf.d/<name>`.
static WEB_APPLICATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^etc/apache2/(?:(?:conf|site)-available/|conf\.d/)(.*)$").expect("valid regex")
});

impl BinaryPackageRule for WebApplicationWorksOnlyWithApache {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if WEB_SERVER.is_match(ctx.package_name()) {
            return;
        }
        let relation = ctx.installation_control().fields().relations_and(&[
            "Pre-Depends",
            "Depends",
            "Recommends",
        ]);
        if !relation.satisfies("apache2") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, webapp) in web_applications(files) {
            ctx.diagnostic(Diagnostic::warn(webapp).with_location(Location {
                path,
                line: None,
                column: None,
            }));
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
            // An alternative is not "only apache2".
            package("apache2 | httpd"),
            package("apache2 | lighttpd | httpd"),
            // Another web server package.
            package("httpd"),
            package("apache2-bin"),
            // Not a web application.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/phpmyfoo/README",
                TestFile::new("doc"),
            )])
            .package("phpmyfoo")
            .field("Depends", "apache2"),
            // The web server itself.
            BinaryPackageCase::from(vec![(
                "etc/apache2/conf-available/apache2.conf",
                TestFile::new(""),
            )])
            .package("apache2")
            .field("Depends", "apache2"),
        ];
        let fail = vec![
            package("apache2"),
            package("apache2, httpd"),
            // Recommends counts too.
            package("httpd").field("Recommends", "apache2"),
            // conf.d and site-available are web application directories as well.
            BinaryPackageCase::from(vec![(
                "etc/apache2/conf.d/phpmyfoo.conf",
                TestFile::new(""),
            )])
            .package("phpmyfoo")
            .field("Depends", "apache2"),
            BinaryPackageCase::from(vec![
                ("etc/apache2/site-available/a.conf", TestFile::new("")),
                ("etc/apache2/conf-available/b.conf", TestFile::new("")),
            ])
            .package("phpmyfoo")
            .field("Depends", "apache2"),
        ];
        BinaryPackageTester::new(WebApplicationWorksOnlyWithApache, pass, fail).test_and_snapshot();
    }
}
