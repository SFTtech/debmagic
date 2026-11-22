use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct WhitespaceAfterContinuationCharacter;

declare_rule! {
    /// A menu item continues over lines, but whitespace follows a backslash.
    ///
    /// The item is still joined, as lintian does, but the backslash should end
    /// the line. Checked on every item, including blank ones made of a
    /// continuation. Extra is empty; Location is the menu file and lintian's
    /// line (the item's last physical line). Udebs are checked too.
    WhitespaceAfterContinuationCharacter,
    code = "LN1192",
    tag = "whitespace-after-continuation-character",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for WhitespaceAfterContinuationCharacter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.whitespace_after_continuation_character) {
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
            // The backslash ends its line.
            menu("?package(example):needs=\"text\" \\\n  title=\"T\" command=\"sh\"\n"),
            // Trailing whitespace on a line that does not continue.
            menu("?package(example):needs=\"text\"   \n"),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/README",
                TestFile::new("?package(example):needs=\"text\" \\  \n"),
            )]),
        ];
        let fail = vec![
            menu("?package(example):needs=\"text\" \\  \n  title=\"T\" command=\"sh\"\n"),
            // Whitespace after the backslash of a continued line, at the end of the item.
            menu(&format!(
                "{GOOD}?package(example):title=\"T\" \\\t\n  command=\"sh\"\n"
            )),
            // A blank item made only of a continuation is still checked.
            menu("\\ \n\n"),
            menu("?package(example):needs=\"text\" \\  \n  title=\"T\" command=\"sh\"\n").udeb(),
        ];
        BinaryPackageTester::new(WhitespaceAfterContinuationCharacter, pass, fail)
            .test_and_snapshot();
    }
}
