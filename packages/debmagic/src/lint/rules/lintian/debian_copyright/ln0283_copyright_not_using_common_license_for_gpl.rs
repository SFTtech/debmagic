use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text::{self, CommonLicenseGap};

pub struct CopyrightNotUsingCommonLicenseForGpl;

declare_rule! {
    /// The copyright file mentions the GPL and does not point at
    /// `/usr/share/common-licenses`.
    ///
    /// GFDL, then LGPL, then GPL, then Apache-2: only the first match is
    /// reported. A Deb822 file is judged on `License` name and body; anything
    /// else is judged as a whole. Mail headers, compatibility wording, and a
    /// misspelled common-licenses directory suppress the Tag. Udebs are not
    /// checked. Extra and Location are empty.
    CopyrightNotUsingCommonLicenseForGpl,
    code = "LN0283",
    tag = "copyright-not-using-common-license-for-gpl",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightNotUsingCommonLicenseForGpl {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if copyright_text::common_license_gap(&contents) == Some(CommonLicenseGap::Gpl) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const DEP5: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn installed(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("usr/share/doc/example", TestFile::directory()),
            ("usr/share/doc/example/copyright", TestFile::new(body)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(installed(
                "GNU General Public License\n/usr/share/common-licenses/GPL-2\n",
            )),
            BinaryPackageCase::from(installed("GNU Lesser General Public License\n")),
            BinaryPackageCase::from(installed(
                "GNU Free Documentation License\nGNU General Public License\n",
            )),
            BinaryPackageCase::from(installed(
                "AFFERO GENERAL PUBLIC LICENSE\nGNU General Public License\n",
            )),
            BinaryPackageCase::from(installed("This software is GPL compatible.\n")),
            BinaryPackageCase::from(installed(
                "See usr/share/common-licences/GPL\nGNU General Public License\n",
            )),
            BinaryPackageCase::from(installed(&format!(
                "{DEP5}\nFiles: *\nCopyright: 2024 Someone, GNU General Public License\nLicense: MIT\n"
            ))),
            BinaryPackageCase::from(installed(&format!(
                "{DEP5}\n# GNU General Public License\nLicense: GPL-2\n"
            ))),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(installed("GNU General Public License\n")).udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed(
                "This program is free software under the GNU General Public License.\n",
            )),
            BinaryPackageCase::from(installed("Released under the GPL.\n")),
            BinaryPackageCase::from(installed(&format!(
                "{DEP5}\nFiles: *\nCopyright: 2024 Someone\nLicense: GPL-2\n The GNU General Public License applies.\n"
            ))),
        ];
        BinaryPackageTester::new(CopyrightNotUsingCommonLicenseForGpl, pass, fail)
            .test_and_snapshot();
    }
}
