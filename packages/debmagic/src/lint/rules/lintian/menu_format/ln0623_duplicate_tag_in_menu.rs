use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DuplicateTagInMenu;

declare_rule! {
    /// A menu item sets the same tag twice.
    ///
    /// Each repeated occurrence is reported once. Extra is the tag. Location
    /// is the menu file and lintian's line. Udebs are checked too.
    DuplicateTagInMenu,
    code = "LN0623",
    tag = "duplicate-tag-in-menu",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DuplicateTagInMenu {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.duplicate_tag_in_menu) {
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

    fn item(extra: &str) -> BinaryPackageCase {
        menu(&format!(
            "?package(example):needs=\"text\" section=\"Applications/System\" title=\"T\" command=\"sh\" {extra}\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            item("longtitle=\"A\""),
            menu(
                "?package(example):needs=\"text\" title=\"A\"\n?package(example):title=\"B\" needs=\"x11\"\n",
            ),
        ];
        let fail = vec![
            item("longtitle=\"A\" longtitle=\"B\""),
            item("needs=\"x11\""),
            item("title=\"T\" title=\"T\""),
            item("title=\"T\" unparsable"),
            BinaryPackageCase::from(vec![(
                "usr/lib/menu/example".to_string(),
                TestFile::new(
                    "?package(example):needs=\"text\" \\\n  needs=\"text\" section=\"x\" title=\"T\" command=\"sh\"\n",
                ),
            )]),
            item("hotkey=\"a\" hotkey=\"b\"").udeb(),
        ];
        BinaryPackageTester::new(DuplicateTagInMenu, pass, fail).test_and_snapshot();
    }
}
