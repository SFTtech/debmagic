use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct Apache2ModuleDoesNotDependOnApache2Api;

declare_rule! {
    /// An Apache module package does not depend on an `apache2-api-<n>`
    /// package.
    ///
    /// Read for each module `usr/lib/apache2/modules/*.so`. The package's
    /// `Pre-Depends`, `Depends` and `Recommends` must name one
    /// `apache2-api-<digits>` alternative, else each module file is reported.
    /// Extra is empty, so the Location (the module file) is part of the
    /// message. The web server packages are exempt.
    Apache2ModuleDoesNotDependOnApache2Api,
    code = "LN0850",
    tag = "apache2-module-does-not-depend-on-apache2-api",
    default_selected = true,
    experimental = false,
}

static APACHE2_API: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^apache2-api-\d+$").expect("valid regex"));

impl BinaryPackageRule for Apache2ModuleDoesNotDependOnApache2Api {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if scan::is_web_server(ctx.package_name()) {
            return;
        }
        let modules = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            scan::modules(files)
        };
        if modules.is_empty() {
            return;
        }
        let depends_on_api = ctx
            .installation_control()
            .fields()
            .relations_and(&["Pre-Depends", "Depends", "Recommends"])
            .alternatives()
            .any(|alternative| APACHE2_API.is_match(alternative.package()));
        if depends_on_api {
            return;
        }
        for (path, _) in modules {
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

    fn module(depends: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/lib/apache2/modules/mod_foo.so".to_string(),
            TestFile::new(""),
        )])
        .field("Depends", depends)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            module("apache2-api-20120211"),
            module("libc6, apache2-api-2"),
            module("apache2-api-20120211 | apache2-bin").package("libapache2-mod-foo"),
            BinaryPackageCase::from(vec![(
                "usr/lib/apache2/modules/mod_foo.so".to_string(),
                TestFile::new(""),
            )])
            .field("Recommends", "apache2-api-20120211"),
            // The web server packages are exempt.
            module("libc6").package("apache2"),
        ];
        let fail = vec![
            module("libc6"),
            module("apache2-api-foo"),
            // Suggests does not count.
            BinaryPackageCase::from(vec![(
                "usr/lib/apache2/modules/mod_foo.so".to_string(),
                TestFile::new(""),
            )])
            .field("Suggests", "apache2-api-20120211"),
        ];
        BinaryPackageTester::new(Apache2ModuleDoesNotDependOnApache2Api, pass, fail)
            .test_and_snapshot();
    }
}
