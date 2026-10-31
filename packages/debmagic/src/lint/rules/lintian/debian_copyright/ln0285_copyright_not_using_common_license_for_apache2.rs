use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text::{self, CommonLicenseGap};

pub struct CopyrightNotUsingCommonLicenseForApache2;

declare_rule! {
    /// The copyright file mentions Apache-2 and does not point at
    /// `/usr/share/common-licenses`.
    ///
    /// Reported only when GFDL, LGPL, and GPL did not already match. On a
    /// Deb822 file the license name matches `apache-2` and the body matches
    /// `Apache License ... Version 2` or `Apache-2`. Udebs are not checked.
    /// Extra and Location are empty.
    CopyrightNotUsingCommonLicenseForApache2,
    code = "LN0285",
    tag = "copyright-not-using-common-license-for-apache2",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightNotUsingCommonLicenseForApache2 {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if copyright_text::common_license_gap(&contents) == Some(CommonLicenseGap::Apache2) {
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
                "Apache License, Version 2.0\n/usr/share/common-licenses/Apache-2.0\n",
            )),
            BinaryPackageCase::from(installed("apache-2\n")),
            BinaryPackageCase::from(installed(
                "GNU General Public License\nApache License, Version 2.0\n",
            )),
            BinaryPackageCase::from(installed(&format!("{DEP5}\nLicense: Apache-2.0\n"))),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(installed("Apache License, Version 2.0\n")).udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed("Apache License, Version 2.0\n")),
            BinaryPackageCase::from(installed("Licensed under Apache-2.\n")),
            BinaryPackageCase::from(installed(&format!(
                "{DEP5}\nLicense: Apache-2.0\n Licensed under the Apache License, Version 2.0\n"
            ))),
        ];
        BinaryPackageTester::new(CopyrightNotUsingCommonLicenseForApache2, pass, fail)
            .test_and_snapshot();
    }
}
