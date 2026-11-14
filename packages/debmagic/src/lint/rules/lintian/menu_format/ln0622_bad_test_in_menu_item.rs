use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct BadTestInMenuItem;

declare_rule! {
    /// A menu item does not start with `?package(name):`.
    ///
    /// Comment and blank lines are skipped, and a continued item is one item.
    /// No extra. Location is the menu file and lintian's line. Udebs are
    /// checked too.
    BadTestInMenuItem,
    code = "LN0622",
    tag = "bad-test-in-menu-item",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BadTestInMenuItem {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.bad_test_in_menu_item) {
            scan::error(ctx, &finding);
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

    const GOOD: &str = "?package(example):needs=\"text\" section=\"Applications/System\" title=\"T\" command=\"sh\"\n";

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            menu(GOOD),
            menu(&format!("# comment\n\n{GOOD}")),
            menu("!C menu-2\nnot a legacy item\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/README",
                TestFile::new("plain text\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/sub/example",
                TestFile::new("plain text\n"),
            )]),
        ];
        let fail = vec![
            menu("package(example):needs=\"text\"\n"),
            menu(&format!("{GOOD}?package(example) needs=\"text\"\n")),
            menu(&format!("# comment\nplain text \\\n  continued\n{GOOD}")),
            BinaryPackageCase::from(vec![(
                "usr/lib/menu/example".to_string(),
                TestFile::new("?package(example\nneeds=\"text\"\n"),
            )]),
            menu("bad\n").udeb(),
        ];
        BinaryPackageTester::new(BadTestInMenuItem, pass, fail).test_and_snapshot();
    }
}
