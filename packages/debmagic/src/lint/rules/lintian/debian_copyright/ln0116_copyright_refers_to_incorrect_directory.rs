use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::installed_copyright;

pub struct CopyrightRefersToIncorrectDirectory;

declare_rule! {
    /// The copyright file spells the common-licenses directory `common-licences`.
    ///
    /// Extra is `usr/share/common-licences`. Location is empty. Udebs are not checked.
    CopyrightRefersToIncorrectDirectory,
    code = "LN0116",
    tag = "copyright-refers-to-incorrect-directory",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightRefersToIncorrectDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let found = installed_copyright::inspect(files, &package);
        for path in found.incorrect_directories {
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
                TestFile::new("usr/share/common-licenses/GPL"),
            ),
        ])];
        let fail = vec![case(vec![
            ("usr/share/doc/example", TestFile::directory()),
            (
                "usr/share/doc/example/copyright",
                TestFile::new("usr/share/common-licences/GPL"),
            ),
        ])];
        BinaryPackageTester::new(CopyrightRefersToIncorrectDirectory, pass, fail)
            .test_and_snapshot();
    }
}
