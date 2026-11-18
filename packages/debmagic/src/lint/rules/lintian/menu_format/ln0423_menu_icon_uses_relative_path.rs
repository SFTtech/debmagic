use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuIconUsesRelativePath;

declare_rule! {
    /// A menu icon path does not start with `/`.
    ///
    /// `icon`, `icon32x32`, and `icon16x16` are checked after the required tags
    /// are present. `none` and an empty value are not this Tag. Extra is the
    /// tag and the path. Location is the menu file and lintian's line. Udebs
    /// are checked too.
    MenuIconUsesRelativePath,
    code = "LN0423",
    tag = "menu-icon-uses-relative-path",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuIconUsesRelativePath {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_icon_uses_relative_path) {
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

    fn item(icons: &str) -> BinaryPackageCase {
        menu(&format!(
            "?package(example):needs=\"text\" section=\"Applications/System\" title=\"T\" command=\"sh\" {icons}\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            item("icon=\"/usr/share/pixmaps/example.xpm\""),
            item("icon=\"none\""),
        ];
        let fail = vec![item(
            "icon=\"non-existent.xpm\" icon16x16=\"non-existent16.xpm\" icon32x32=\"non-existent32.xpm\"",
        )];
        BinaryPackageTester::new(MenuIconUsesRelativePath, pass, fail).test_and_snapshot();
    }
}
