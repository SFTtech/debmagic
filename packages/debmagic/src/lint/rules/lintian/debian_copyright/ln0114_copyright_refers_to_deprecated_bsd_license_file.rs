use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::installed_copyright;

pub struct CopyrightRefersToDeprecatedBsdLicenseFile;

declare_rule! {
    /// The copyright file refers to `/usr/share/common-licenses/BSD`.
    ///
    /// Extra and Location are empty. Udebs are not checked.
    CopyrightRefersToDeprecatedBsdLicenseFile,
    code = "LN0114",
    tag = "copyright-refers-to-deprecated-bsd-license-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightRefersToDeprecatedBsdLicenseFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let found = installed_copyright::inspect(files, &package);
        if found.deprecated_bsd {
            ctx.diagnostic(Diagnostic::warn(""));
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
                TestFile::new("BSD-3-Clause"),
            ),
        ])];
        let fail = vec![case(vec![
            ("usr/share/doc/example", TestFile::directory()),
            (
                "usr/share/doc/example/copyright",
                TestFile::new("See /usr/share/common-licenses/BSD"),
            ),
        ])];
        BinaryPackageTester::new(CopyrightRefersToDeprecatedBsdLicenseFile, pass, fail)
            .test_and_snapshot();
    }
}
