use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct UnparsableMenuItem;

declare_rule! {
    /// A menu item cannot be read as a series of `tag=value` pairs.
    ///
    /// Only items that start with `?package(name):` are read, and text after
    /// the last pair that is not whitespace (such as a stray word or an
    /// unterminated quote) makes the item unparsable. Lintian scans for pairs
    /// without anchoring, so a bare token before a later pair is skipped, not
    /// reported. Extra is empty; Location is the menu file and lintian's line.
    /// Udebs are checked too.
    UnparsableMenuItem,
    code = "LN1191",
    tag = "unparsable-menu-item",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnparsableMenuItem {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.unparsable_menu_item) {
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
            // Unquoted values and escapes inside quotes parse.
            menu("?package(example):title=T title=\"a \\\"b\\\"\"\n"),
            // A bare token before a later pair is skipped by lintian's search.
            menu("?package(example):stray needs=\"text\"\n"),
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
            menu(&format!("{GOOD}?package(example):needs=\"text\" junk\n")),
            menu("?package(example):title=\"open\n"),
            menu("?package(example):needs=\"text\" \\\n  title=\"T\" junk\n"),
            BinaryPackageCase::from(vec![(
                "usr/lib/menu/example".to_string(),
                TestFile::new("?package(example):needs=\"text\" junk\n"),
            )]),
            menu("?package(example):needs=\"text\" junk\n").udeb(),
        ];
        BinaryPackageTester::new(UnparsableMenuItem, pass, fail).test_and_snapshot();
    }
}
