use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::pear::PearSource;

pub struct PearPackageWithoutPkgPhpToolsBuilddep;

declare_rule! {
    /// A PEAR or PECL package (its `package.xml` or `package2.xml`) has neither
    /// `pkg-php-tools` nor `dh-sequence-phppear` in Build-Depends.
    ///
    /// Only when a `.php` file is shipped. The Diagnostic has no extra and no
    /// Location.
    PearPackageWithoutPkgPhpToolsBuilddep,
    code = "LN1286",
    tag = "pear-package-without-pkg-php-tools-builddep",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PearPackageWithoutPkgPhpToolsBuilddep {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(pear) = PearSource::new(ctx) else {
            return;
        };
        if pear.is_pear() && !pear.has_pear_builddep() {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    fn tree(build_depends: &str, package: Option<&str>) -> TestCase {
        let mut files = vec![
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: foo\nBuild-Depends: {build_depends}\n\nPackage: foo\nArchitecture: all\nDescription: foo\n extra\n"
                )),
            ),
            ("foo.php", TestFile::new("<?php\n")),
        ];
        if let Some(name) = package {
            files.push((name, TestFile::new("<package/>\n")));
        }
        TestCase::source_tree(files)
    }

    #[test]
    fn source() {
        let pass = vec![
            tree(
                "debhelper-compat (= 13), pkg-php-tools",
                Some("package.xml"),
            ),
            tree(
                "debhelper-compat (= 13), dh-sequence-phppear",
                Some("package2.xml"),
            ),
            // Not a PEAR package.
            tree("debhelper-compat (= 13)", None),
            tree("debhelper-compat (= 13)", Some("composer.json")),
        ];
        let fail = vec![
            tree("debhelper-compat (= 13)", Some("package.xml")),
            tree(
                "debhelper-compat (= 13), dh-sequence-phpcomposer",
                Some("package2.xml"),
            ),
        ];
        SourceTreeTester::new(PearPackageWithoutPkgPhpToolsBuilddep, pass, fail)
            .test_and_snapshot();
    }
}
