use std::path::PathBuf;

use regex::Regex;

use super::rules_file::{logical_rules, rules_files};
use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct UdevRuleMissingSubsystem;

declare_rule! {
    /// The Binary package ships a udev rule in `usr/lib/udev/rules.d/` or
    /// `lib/udev/rules.d/` that matches `ATTR{idVendor}` and `ATTR{idProduct}`
    /// without a `SUBSYSTEM` key. Vendor and product IDs are subsystem specific.
    ///
    /// Only readable children of the rules directories are read (symlinks followed
    /// inside the tree). A rule that ends on a line with `\` continues onto the next.
    /// Comments are skipped, and rules after a `GOTO` on a `SUBSYSTEM!=` line
    /// (until the next `LABEL`) are not reported. Location is the line where the
    /// logical rule ends.
    UdevRuleMissingSubsystem,
    code = "LN1226",
    tag = "udev-rule-missing-subsystem",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UdevRuleMissingSubsystem {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let vendor = Regex::new(r#"ATTR\{idVendor\}=="[0-9a-fA-F]+""#).expect("valid regex");
        let product = Regex::new(r#"ATTR\{idProduct\}=="[0-9a-fA-F]*""#).expect("valid regex");
        let subsystem = Regex::new(r#"SUBSYSTEM=="[^"]+""#).expect("valid regex");

        let mut hits: Vec<(PathBuf, u32)> = Vec::new();
        for file in rules_files(ctx) {
            if !file.readable {
                continue;
            }
            for rule in logical_rules(&file.text) {
                if vendor.is_match(&rule.text)
                    && product.is_match(&rule.text)
                    && !rule.in_goto
                    && !subsystem.is_match(&rule.text)
                {
                    hits.push((file.path.clone(), rule.position));
                }
            }
        }
        for (path, line) in hits {
            ctx.diagnostic(
                Diagnostic::warn("vendor/product matching missing SUBSYSTEM specifier")
                    .with_location(Location {
                        path,
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const RULES: &str = "usr/lib/udev/rules.d/60-foo.rules";

    fn file(content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(RULES, TestFile::new(content))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            file(
                "SUBSYSTEM==\"usb\", ATTR{idVendor}==\"0001\", ATTR{idProduct}==\"0002\", MODE=\"0666\"\n",
            ),
            // Only one of the two keys.
            file("ATTR{idVendor}==\"0001\", MODE=\"0666\"\n"),
            // Comment lines are not rules.
            file("# ATTR{idVendor}==\"0001\", ATTR{idProduct}==\"0002\"\n"),
            // After a GOTO on SUBSYSTEM!=, until the next LABEL.
            file(
                "SUBSYSTEM!=\"usb\", GOTO=\"t\"\nATTR{idVendor}==\"0001\", ATTR{idProduct}==\"0002\"\nLABEL=\"t\"\n",
            ),
            // Not in a rules directory.
            BinaryPackageCase::from(vec![(
                "usr/lib/udev/other/60-foo.rules",
                TestFile::new("ATTR{idVendor}==\"0001\", ATTR{idProduct}==\"0002\"\n"),
            )]),
            // A directory entry is not read.
            BinaryPackageCase::from(vec![("usr/lib/udev/rules.d/sub", TestFile::directory())]),
            // Only a dangling symlink; not readable, so not read.
            BinaryPackageCase::from(vec![(RULES, TestFile::symlink("/nowhere/60-foo.rules"))]),
        ];
        let fail = vec![
            file("ATTR{idVendor}==\"0001\", ATTR{idProduct}==\"0002\", MODE=\"0666\"\n"),
            // The reported line is where the logical rule ends.
            file("\n\nATTR{idVendor}==\"0001\", \\\n    ATTR{idProduct}==\"0002\"\n"),
            // Empty product value still matches the pattern.
            file("ATTR{idVendor}==\"00ff\", ATTR{idProduct}==\"\"\n"),
            // Under lib/ as well as usr/lib/.
            BinaryPackageCase::from(vec![(
                "lib/udev/rules.d/60-foo.rules",
                TestFile::new("ATTR{idVendor}==\"0001\", ATTR{idProduct}==\"0002\"\n"),
            )]),
            // A symlink inside the tree to a readable rule file.
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/udev/rules.d/60-foo.rules",
                    TestFile::symlink("60-real.rules"),
                ),
                (
                    "usr/lib/udev/rules.d/60-real.rules",
                    TestFile::new("ATTR{idVendor}==\"0001\", ATTR{idProduct}==\"0002\"\n"),
                ),
            ]),
            // A udeb is still a Binary package.
            file("ATTR{idVendor}==\"0001\", ATTR{idProduct}==\"0002\"\n").udeb(),
        ];
        BinaryPackageTester::new(UdevRuleMissingSubsystem, pass, fail).test_and_snapshot();
    }
}
