use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct UnquotedStringInMenuItem;

declare_rule! {
    /// A menu item's tag value is not a quoted string.
    ///
    /// Reported while the item is parsed, including items that lack a required
    /// tag. Extra is the tag name. Location is the menu file and lintian's
    /// line. Udebs are checked too.
    UnquotedStringInMenuItem,
    code = "LN0418",
    tag = "unquoted-string-in-menu-item",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnquotedStringInMenuItem {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.unquoted_string_in_menu_item) {
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

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            menu(
                "?package(example):needs=\"text\" section=\"Applications/System\" title=\"T\" command=\"sh\"\n",
            ),
            menu("# comment\n"),
        ];
        let fail = vec![
            menu("?package(example):needs=text title=\"Hello\" command=\"/usr/bin/hello\"\n"),
            menu(
                "?package(example):needs=text section=\"Applications/System\" title=\"T\" command=\"sh\"\n",
            ),
        ];
        BinaryPackageTester::new(UnquotedStringInMenuItem, pass, fail).test_and_snapshot();
    }
}
