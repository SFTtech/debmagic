use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct BadMenuItem;

declare_rule! {
    /// `Installer-Menu-Item` of a udeb is not a non-negative integer.
    ///
    /// Reads `Installer-Menu-Item` of the Installation control of a udeb; debs
    /// are not checked. The unfolded value must be only ASCII digits, an empty
    /// value included. Extra is the unfolded value.
    BadMenuItem,
    code = "LN0836",
    tag = "bad-menu-item",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BadMenuItem {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !ctx.is_udeb() {
            return;
        }
        let Some(value) = ctx
            .installation_control()
            .fields()
            .unfolded_value("Installer-Menu-Item")
        else {
            return;
        };
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            ctx.diagnostic(Diagnostic::error(value));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn menu_item(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Installer-Menu-Item", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            menu_item("10").udeb(),
            menu_item("0").udeb(),
            // Only udebs have an installer menu item.
            menu_item("network"),
            menu_item("network -1"),
        ];
        let fail = vec![
            menu_item("network").udeb(),
            menu_item("-1").udeb(),
            menu_item("1a").udeb(),
            menu_item("").udeb(),
            menu_item("10\n 20").udeb(),
        ];
        BinaryPackageTester::new(BadMenuItem, pass, fail).test_and_snapshot();
    }
}
