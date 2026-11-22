use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::rules_file::rules_files;

pub struct UdevRuleUnreadable;

declare_rule! {
    /// The Binary package has an entry in `usr/lib/udev/rules.d/` or
    /// `lib/udev/rules.d/` that is not a readable file: a directory, a dangling
    /// symlink, a symlink that leaves the tree, or a device or pipe. Extra is
    /// empty; the path is the Location.
    UdevRuleUnreadable,
    code = "LN1228",
    tag = "udev-rule-unreadable",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UdevRuleUnreadable {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let paths: Vec<_> = rules_files(ctx)
            .into_iter()
            .filter(|file| !file.readable)
            .map(|file| file.path)
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/lib/udev/rules.d/60-foo.rules",
                TestFile::new("ACTION==\"add\"\n"),
            )]),
            // A symlink to a readable file in the tree is fine.
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/udev/rules.d/60-foo.rules",
                    TestFile::symlink("60-real.rules"),
                ),
                (
                    "usr/lib/udev/rules.d/60-real.rules",
                    TestFile::new("ACTION==\"add\"\n"),
                ),
            ]),
            // Not in a rules directory.
            BinaryPackageCase::from(vec![(
                "usr/lib/udev/other/60-foo.rules",
                TestFile::symlink("/nowhere"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/udev/rules.d/60-dangling-symlink.rules",
                TestFile::symlink("/nowhere/at/all"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/udev/rules.d/60-dangling.rules",
                TestFile::symlink("missing.rules"),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/udev/rules.d/sub", TestFile::directory())]),
            BinaryPackageCase::from(vec![("lib/udev/rules.d/fifo", TestFile::fifo())]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![(
                "usr/lib/udev/rules.d/60-dangling.rules",
                TestFile::symlink("missing.rules"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(UdevRuleUnreadable, pass, fail).test_and_snapshot();
    }
}
