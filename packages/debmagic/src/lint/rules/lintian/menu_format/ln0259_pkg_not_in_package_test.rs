use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PkgNotInPackageTest;

declare_rule! {
    /// A menu item's `?package` test does not name this package.
    ///
    /// The extra is the raw test, commas and spacing included. Reported even
    /// when the rest of the line is unparsable or missing a required tag.
    /// Location is the menu file and lintian's line. A `!C menu-2` file is
    /// skipped.
    PkgNotInPackageTest,
    code = "LN0259",
    tag = "pkg-not-in-package-test",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PkgNotInPackageTest {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.pkg_not_in_package_test) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn menu(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/menu/example", TestFile::new(body))])
    }

    const REST: &str = "needs=\"text\" section=\"Help\" title=\"T\" command=\"sh\"";

    #[test]
    fn test() {
        let pass = vec![
            menu(&format!("?package(example):{REST}\n")),
            menu(&format!("?package(example, other):{REST}\n")),
            menu(&format!("?package(other, example):{REST}\n")),
            menu(&format!("?package(example,example):{REST}\n")),
            menu(&format!("!C menu-2\n?package(other):{REST}\n")),
            menu("this is not a package test\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/README".to_string(),
                TestFile::new(format!("?package(other):{REST}\n")),
            )]),
            menu("# only a comment\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/extra/item".to_string(),
                TestFile::new(format!("?package(other):{REST}\n")),
            )]),
        ];
        let fail = vec![
            menu(&format!("?package(wrong-package):{REST}\n")),
            menu(&format!("?package(other, third):{REST}\n")),
            menu("?package(other): this is junk\n"),
            menu("?package(other):title=\"T\" command=\"sh\"\n"),
            menu(&format!("# comment\n\n?package(other):{REST}\n")),
            BinaryPackageCase::from(vec![(
                "usr/lib/menu/example".to_string(),
                TestFile::new(format!("?package(other):{REST}\n")),
            )]),
            menu(
                "?package(unknown):\\ \n  needs=\"text\" section=\"Help\" title=\"T\" command=\"sh\"\n",
            ),
            menu(&format!("?package(other):{REST}\n")).udeb(),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/example".to_string(),
                TestFile::new(format!("?package(other):{REST}\n")).hardlink(),
            )]),
        ];
        BinaryPackageTester::new(PkgNotInPackageTest, pass, fail).test_and_snapshot();
    }
}
