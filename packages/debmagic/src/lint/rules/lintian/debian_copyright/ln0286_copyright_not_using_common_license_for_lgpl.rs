use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text::{self, CommonLicenseGap};

pub struct CopyrightNotUsingCommonLicenseForLgpl;

declare_rule! {
    /// The copyright file mentions the LGPL and does not point at
    /// `/usr/share/common-licenses`.
    ///
    /// Reported only when GFDL did not already match, and in preference to GPL
    /// and Apache-2. A line break inside `GNU Lesser General Public License`
    /// does not match. Udebs are not checked. Extra and Location are empty.
    CopyrightNotUsingCommonLicenseForLgpl,
    code = "LN0286",
    tag = "copyright-not-using-common-license-for-lgpl",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightNotUsingCommonLicenseForLgpl {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if copyright_text::common_license_gap(&contents) == Some(CommonLicenseGap::Lgpl) {
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
                "GNU Lesser General Public License\n/usr/share/common-licenses/LGPL-3\n",
            )),
            BinaryPackageCase::from(installed("GNU General Public License\n")),
            BinaryPackageCase::from(installed("lgpl\n")),
            // A line break keeps the LGPL phrase from matching.
            BinaryPackageCase::from(installed(
                "use the GNU Lesser General\nPublic License instead\n",
            )),
            BinaryPackageCase::from(installed(
                "GNU Free Documentation License\nGNU Lesser General Public License\n",
            )),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(installed("LGPL\n")).udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed("GNU Lesser General Public License\n")),
            BinaryPackageCase::from(installed("GNU Library General Public License\n")),
            BinaryPackageCase::from(installed("Distributed under the LGPL.\n")),
            BinaryPackageCase::from(installed(&format!(
                "{DEP5}\nLicense: LGPL-2.1\n GNU Lesser General Public License.\n"
            ))),
        ];
        BinaryPackageTester::new(CopyrightNotUsingCommonLicenseForLgpl, pass, fail)
            .test_and_snapshot();
    }
}
