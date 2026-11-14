use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text::{self, CommonLicenseGap};

pub struct CopyrightNotUsingCommonLicenseForGfdl;

declare_rule! {
    /// The copyright file mentions the GFDL and does not point at
    /// `/usr/share/common-licenses`.
    ///
    /// GFDL is the first of the GFDL, LGPL, GPL, Apache-2 chain, so it wins
    /// over the others. A Deb822 file is judged on `License` name and body;
    /// anything else is judged as a whole. Mail headers, compatibility wording,
    /// GFDL 1.1, and a misspelled common-licenses directory suppress the Tag.
    /// Udebs are not checked. Extra and Location are empty.
    CopyrightNotUsingCommonLicenseForGfdl,
    code = "LN0607",
    tag = "copyright-not-using-common-license-for-gfdl",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightNotUsingCommonLicenseForGfdl {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if copyright_text::common_license_gap(&contents) == Some(CommonLicenseGap::Gfdl) {
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
                "GNU Free Documentation License\n/usr/share/common-licenses/GFDL-1.2\n",
            )),
            BinaryPackageCase::from(installed("Copyright 2024 Someone\n")),
            // GFDL 1.1 is exempt.
            BinaryPackageCase::from(installed("GNU Free Documentation License, Version 1.1\n")),
            // A Deb822 license field with a GFDL name but no body is not judged.
            BinaryPackageCase::from(installed(&format!(
                "{DEP5}\nFiles: *\nCopyright: 2024 Someone\nLicense: GFDL-1.2\n"
            ))),
            BinaryPackageCase::from(installed("GNU Free Documentation License\n")).udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed(
                "Copyright 2024\nGNU Free Documentation License\n",
            )),
            BinaryPackageCase::from(installed("Licensed under the GFDL.\n")),
            BinaryPackageCase::from(installed(&format!(
                "{DEP5}\nFiles: *\nCopyright: 2024 Someone\nLicense: GFDL-1.2\n Permission is granted under the GNU Free Documentation License.\n"
            ))),
            // GFDL wins over the GPL in the same file.
            BinaryPackageCase::from(installed("GFDL and GNU General Public License\n")),
        ];
        BinaryPackageTester::new(CopyrightNotUsingCommonLicenseForGfdl, pass, fail)
            .test_and_snapshot();
    }
}
