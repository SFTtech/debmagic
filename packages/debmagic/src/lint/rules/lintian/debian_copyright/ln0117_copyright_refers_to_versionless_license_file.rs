use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::installed_copyright;

pub struct CopyrightRefersToVersionlessLicenseFile;

declare_rule! {
    /// The copyright file cites versionless GPL, LGPL, or GFDL and says "version".
    ///
    /// "or later" and a `License:` line that already carries a version silence
    /// it. Extra is the reference without a leading slash. Location is empty.
    /// Udebs are not checked.
    CopyrightRefersToVersionlessLicenseFile,
    code = "LN0117",
    tag = "copyright-refers-to-versionless-license-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightRefersToVersionlessLicenseFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let found = installed_copyright::inspect(files, &package);
        for reference in found.versionless {
            ctx.diagnostic(Diagnostic::warn(reference));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(entries: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![case(vec![
            ("usr/share/doc/example", TestFile::directory()),
            (
                "usr/share/doc/example/copyright",
                TestFile::new("usr/share/common-licenses/GPL or later"),
            ),
        ])];
        let fail = vec![case(vec![
            ("usr/share/doc/example", TestFile::directory()),
            (
                "usr/share/doc/example/copyright",
                TestFile::new("the GPL version in usr/share/common-licenses/GPL."),
            ),
        ])];
        BinaryPackageTester::new(CopyrightRefersToVersionlessLicenseFile, pass, fail)
            .test_and_snapshot();
    }
}
