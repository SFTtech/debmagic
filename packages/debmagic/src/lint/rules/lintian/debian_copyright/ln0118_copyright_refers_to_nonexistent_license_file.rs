use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::installed_copyright;

pub struct CopyrightRefersToNonexistentLicenseFile;

declare_rule! {
    /// The copyright file extends GPL, LGPL, or GFDL with a letter or digit.
    ///
    /// Extra is the reference plus that character. Location is empty.
    /// Udebs are not checked.
    CopyrightRefersToNonexistentLicenseFile,
    code = "LN0118",
    tag = "copyright-refers-to-nonexistent-license-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightRefersToNonexistentLicenseFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let found = installed_copyright::inspect(files, &package);
        for reference in found.nonexistent {
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
                TestFile::new("usr/share/common-licenses/GPL-2"),
            ),
        ])];
        let fail = vec![case(vec![
            ("usr/share/doc/example", TestFile::directory()),
            (
                "usr/share/doc/example/copyright",
                TestFile::new("usr/share/common-licenses/GPL2"),
            ),
        ])];
        BinaryPackageTester::new(CopyrightRefersToNonexistentLicenseFile, pass, fail)
            .test_and_snapshot();
    }
}
