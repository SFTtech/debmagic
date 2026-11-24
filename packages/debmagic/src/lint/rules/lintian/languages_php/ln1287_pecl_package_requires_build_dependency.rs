use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::pear::{PACKAGE_XML, PearSource};

pub struct PeclPackageRequiresBuildDependency;

declare_rule! {
    /// A PECL package (a `package.xml` whose release is `extsrc`) does not
    /// have `php-dev` and `dh-php` in Build-Depends. Each missing one is
    /// reported with the package name as extra and `package.xml` as Location.
    ///
    /// Only when `package.xml` is a regular file (not `package2.xml`) and the
    /// PEAR packaging is set up (Build-Depends satisfies `pkg-php-tools` or
    /// `dh-sequence-phppear`); without that, LN1286 reports the missing
    /// packaging instead. The release kind is read as lintian does: the first
    /// line that opens a `<...release>` or `<bundle>` element.
    PeclPackageRequiresBuildDependency,
    code = "LN1287",
    tag = "pecl-package-requires-build-dependency",
    default_selected = true,
    experimental = false,
}

const BUILD_DEPENDENCIES: &[&str] = &["php-dev", "dh-php"];

impl SourceRule for PeclPackageRequiresBuildDependency {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(pear) = PearSource::new(ctx) else {
            return;
        };
        if !pear.package_xml || !pear.has_pear_builddep() || pear.package_type != "extsrc" {
            return;
        }
        for dependency in BUILD_DEPENDENCIES {
            if !pear.build_depends.satisfies(dependency) {
                ctx.diagnostic(Diagnostic::warn(*dependency).with_location(Location {
                    path: PathBuf::from(PACKAGE_XML),
                    line: None,
                    column: None,
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    const PECL_FILE: &str = "<package>\n <name>foo</name>\n <extsrcrelease/>\n</package>\n";

    fn tree(build_depends: &str, package: (&str, &str)) -> TestCase {
        TestCase::source_tree([
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: foo\nBuild-Depends: {build_depends}\n\nPackage: foo\nArchitecture: all\nDescription: foo\n extra\n"
                )),
            ),
            (package.0, TestFile::new(package.1.to_string())),
            ("foo.php", TestFile::new("<?php\n")),
        ])
    }

    #[test]
    fn source() {
        let pass = vec![
            tree(
                "debhelper-compat (= 13), pkg-php-tools, php-dev, dh-php",
                ("package.xml", PECL_FILE),
            ),
            // A PEAR package is not PECL.
            tree(
                "pkg-php-tools",
                ("package.xml", "<package><phprelease/></package>\n"),
            ),
            // Only package.xml is read for the release kind.
            tree("pkg-php-tools", ("package2.xml", PECL_FILE)),
            // Without the PEAR packaging LN1286 reports instead.
            tree("debhelper-compat (= 13)", ("package.xml", PECL_FILE)),
        ];
        let fail = vec![
            tree("pkg-php-tools, dh-php", ("package.xml", PECL_FILE)),
            tree("pkg-php-tools, php-dev", ("package.xml", PECL_FILE)),
            tree("dh-sequence-phppear", ("package.xml", PECL_FILE)),
        ];
        SourceTreeTester::new(PeclPackageRequiresBuildDependency, pass, fail).test_and_snapshot();
    }
}
