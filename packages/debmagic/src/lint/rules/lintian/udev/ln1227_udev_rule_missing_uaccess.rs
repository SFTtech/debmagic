use std::path::PathBuf;

use super::rules_file::{logical_rules, rules_files};
use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct UdevRuleMissingUaccess;

declare_rule! {
    /// The Binary package ships a udev rule in `usr/lib/udev/rules.d/` or
    /// `lib/udev/rules.d/` that makes a USB device accessible to users (`GROUP="plugdev"`
    /// or `MODE="0666"` on `SUBSYSTEM=="usb"`) without `TAG+="uaccess"`.
    ///
    /// Rules that set one of the `ENV{...}` markers handled by
    /// `70-uaccess.rules` (for example `ID_CDROM`, `ID_GPHOTO2`) are not reported.
    /// Only readable children of the rules directories are read (symlinks followed
    /// inside the tree). Location is the line where the logical rule ends.
    UdevRuleMissingUaccess,
    code = "LN1227",
    tag = "udev-rule-missing-uaccess",
    default_selected = true,
    experimental = false,
}

/// The `ENV{...}` markers that make `70-uaccess.rules` grant access instead.
const UACCESS_ENV_MARKERS: [&str; 14] = [
    "ENV{COLOR_MEASUREMENT_DEVICE}",
    "ENV{DDC_DEVICE}",
    "ENV{ID_CDROM}",
    "ENV{ID_FFADO}",
    "ENV{ID_GPHOTO2}",
    "ENV{ID_HPLIP}",
    "ENV{ID_INPUT_JOYSTICK}",
    "ENV{ID_MAKER_TOOL}",
    "ENV{ID_MEDIA_PLAYER}",
    "ENV{ID_PDA}",
    "ENV{ID_REMOTE_CONTROL}",
    "ENV{ID_SECURITY_TOKEN}",
    "ENV{ID_SMARTCARD_READER}",
    "ENV{ID_SOFTWARE_RADIO}",
];

fn is_missing_uaccess(rule: &str) -> bool {
    (rule.contains("SUBSYSTEM==\"usb\"")
        && (rule.contains("GROUP=\"plugdev\"") || rule.contains("MODE=\"0666\"")))
        && !UACCESS_ENV_MARKERS
            .iter()
            .any(|marker| rule.contains(marker))
        && !rule.contains("TAG+=\"uaccess\"")
}

impl BinaryPackageRule for UdevRuleMissingUaccess {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let mut hits: Vec<(PathBuf, u32)> = Vec::new();
        for file in rules_files(ctx) {
            if !file.readable {
                continue;
            }
            for rule in logical_rules(&file.text) {
                if is_missing_uaccess(&rule.text) {
                    hits.push((file.path.clone(), rule.position));
                }
            }
        }
        for (path, line) in hits {
            ctx.diagnostic(
                Diagnostic::warn("user accessible device missing TAG+=\"uaccess\"").with_location(
                    Location {
                        path,
                        line: Some(line),
                        column: None,
                    },
                ),
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
            file("SUBSYSTEM==\"usb\", ATTR{idVendor}==\"0001\", MODE=\"0660\", TAG+=\"uaccess\"\n"),
            // Without SUBSYSTEM=="usb".
            file("ACTION==\"add\", MODE=\"0666\"\n"),
            // Neither plugdev nor 0666.
            file("SUBSYSTEM==\"usb\", MODE=\"0600\"\n"),
            // A 70-uaccess.rules marker exempts the rule.
            file("SUBSYSTEM==\"usb\", ENV{ID_GPHOTO2}==\"1\", MODE=\"0666\"\n"),
            // Comments are not rules.
            file("# SUBSYSTEM==\"usb\", MODE=\"0666\"\n"),
            // Not in a rules directory.
            BinaryPackageCase::from(vec![(
                "usr/lib/udev/other/60-foo.rules",
                TestFile::new("SUBSYSTEM==\"usb\", MODE=\"0666\"\n"),
            )]),
            // A dangling symlink is not read.
            BinaryPackageCase::from(vec![(RULES, TestFile::symlink("/nowhere"))]),
        ];
        let fail = vec![
            file("SUBSYSTEM==\"usb\", ATTR{idVendor}==\"0001\", MODE=\"0666\"\n"),
            file("SUBSYSTEM==\"usb\", GROUP=\"plugdev\", MODE=\"0660\"\n"),
            // The reported line is where the logical rule ends.
            file("\n\nSUBSYSTEM==\"usb\", \\\n    MODE=\"0660\", GROUP=\"plugdev\"\n"),
            // Under lib/ as well as usr/lib/.
            BinaryPackageCase::from(vec![(
                "lib/udev/rules.d/60-foo.rules",
                TestFile::new("SUBSYSTEM==\"usb\", MODE=\"0666\"\n"),
            )]),
            // A udeb is still a Binary package.
            file("SUBSYSTEM==\"usb\", MODE=\"0666\"\n").udeb(),
        ];
        BinaryPackageTester::new(UdevRuleMissingUaccess, pass, fail).test_and_snapshot();
    }
}
