use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::installed_copyright;

pub struct CopyrightRefersToCompressedLicense;

declare_rule! {
    /// The copyright file points at a `.gz` under `usr/share/common-licenses`.
    ///
    /// The license name must be one lintian ships in `common-licenses`.
    /// Extra is the path, including `.gz`. Location is empty. Udebs are not checked.
    CopyrightRefersToCompressedLicense,
    code = "LN0115",
    tag = "copyright-refers-to-compressed-license",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightRefersToCompressedLicense {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let found = installed_copyright::inspect(files, &package);
        for path in found.compressed_licenses {
            ctx.diagnostic(Diagnostic::error(path));
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
                TestFile::new("usr/share/common-licenses/NotALicense.gz"),
            ),
        ])];
        let fail = vec![case(vec![
            ("usr/share/doc/example", TestFile::directory()),
            (
                "usr/share/doc/example/copyright",
                TestFile::new("see usr/share/common-licenses/GPL-2.gz please"),
            ),
        ])];
        BinaryPackageTester::new(CopyrightRefersToCompressedLicense, pass, fail)
            .test_and_snapshot();
    }
}
