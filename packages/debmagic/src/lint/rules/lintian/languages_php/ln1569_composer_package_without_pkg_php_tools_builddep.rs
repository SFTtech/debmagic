use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::pear::{COMPOSER_JSON, PearSource};

pub struct ComposerPackageWithoutPkgPhpToolsBuilddep;

declare_rule! {
    /// A `composer.json` is shipped, Build-Depends neither satisfies
    /// `pkg-php-tools` nor `dh-sequence-phpcomposer`, and the package is not
    /// PEAR (no `package.xml` or `package2.xml`).
    ///
    /// Only when a `.php` file is shipped, as the Check's other Rules. The
    /// `composer.json` is the Location; the extra is empty. Severity is info
    /// and the Tag is not selected by default, as in lintian.
    ComposerPackageWithoutPkgPhpToolsBuilddep,
    code = "LN1569",
    tag = "composer-package-without-pkg-php-tools-builddep",
    default_selected = false,
    experimental = false,
}

impl SourceRule for ComposerPackageWithoutPkgPhpToolsBuilddep {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(pear) = PearSource::new(ctx) else {
            return;
        };
        let composer_prerequisite =
            pear.has_pkg_php_tools() || pear.build_depends.satisfies("dh-sequence-phpcomposer");
        if pear.composer_json && !composer_prerequisite && !pear.is_pear() {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_location(Location {
                        path: COMPOSER_JSON.into(),
                        line: None,
                        column: None,
                    })
                    .with_severity(Severity::Info),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    fn tree(build_depends: &str, extra: &[&str], php: bool) -> TestCase {
        let mut files = vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: foo\nBuild-Depends: {build_depends}\n\nPackage: foo\nArchitecture: all\nDescription: foo\n extra\n"
            )),
        )];
        if php {
            files.push(("foo.php", TestFile::new("<?php\n")));
        }
        for name in extra {
            files.push((name, TestFile::new("{}\n")));
        }
        TestCase::source_tree(files)
    }

    #[test]
    fn source() {
        let pass = vec![
            // Without a .php file the Check does not run.
            tree("debhelper-compat (= 13)", &["composer.json"], false),
            tree(
                "debhelper-compat (= 13), pkg-php-tools",
                &["composer.json"],
                true,
            ),
            tree(
                "debhelper-compat (= 13), dh-sequence-phpcomposer",
                &["composer.json"],
                true,
            ),
            // A PEAR package is not reported here.
            tree(
                "debhelper-compat (= 13)",
                &["composer.json", "package.xml"],
                true,
            ),
            tree(
                "debhelper-compat (= 13)",
                &["composer.json", "package2.xml"],
                true,
            ),
            // No composer.json.
            tree("debhelper-compat (= 13)", &[], true),
            // The lookup is exact: COMPOSER.JSON is not composer.json.
            tree("debhelper-compat (= 13)", &["COMPOSER.JSON"], true),
        ];
        let fail = vec![
            tree("debhelper-compat (= 13)", &["composer.json"], true),
            tree(
                "debhelper-compat (= 13), dh-sequence-phppear",
                &["composer.json"],
                true,
            ),
            tree(
                "debhelper-compat (= 13)",
                &["composer.json", "channel.xml"],
                true,
            ),
        ];
        SourceTreeTester::new(ComposerPackageWithoutPkgPhpToolsBuilddep, pass, fail)
            .test_and_snapshot();
    }
}
