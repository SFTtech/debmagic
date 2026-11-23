use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::installed_copyright;

pub struct CopyrightRefersToSymlinkLicense;

declare_rule! {
    /// The copyright file cites a versionless common license, which is a
    /// symlink to the latest version, and says "or later" or similar.
    ///
    /// The reference is `usr/share/common-licenses/GPL` or `LGPL`, or `GFDL`,
    /// with no version suffix, in a copyright file that also has a later-version
    /// phrase, `as Perl itself`, or a Perl license (lintian's symlink branch of
    /// the common-license references). Extra is the reference without a leading
    /// slash; the Location is empty. Pedantic, not selected by default. Udebs
    /// are not checked.
    CopyrightRefersToSymlinkLicense,
    code = "LN1642",
    tag = "copyright-refers-to-symlink-license",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for CopyrightRefersToSymlinkLicense {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let found = installed_copyright::inspect(files, &package);
        for reference in found.symlink_license {
            ctx.diagnostic(Diagnostic::warn(reference).with_severity(Severity::Pedantic));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn installed(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("usr/share/doc/example", TestFile::directory()),
            ("usr/share/doc/example/copyright", TestFile::new(body)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            // A versioned reference is fine.
            BinaryPackageCase::from(installed(
                "License: GPL-2+\n On Debian systems see /usr/share/common-licenses/GPL-2\n",
            )),
            // No later-version phrase: this is the versionless Tag's case.
            BinaryPackageCase::from(installed(
                "See /usr/share/common-licenses/GPL for the version 2 text\n",
            )),
            BinaryPackageCase::from(installed(
                "Licensed under /usr/share/common-licenses/GPL or any later version.\n",
            ))
            .udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed(
                "On Debian see /usr/share/common-licenses/GPL, or any later version.\n",
            )),
            BinaryPackageCase::from(installed(
                "This is as Perl itself, see /usr/share/common-licenses/LGPL\n",
            )),
        ];
        BinaryPackageTester::new(CopyrightRefersToSymlinkLicense, pass, fail).test_and_snapshot();
    }
}
