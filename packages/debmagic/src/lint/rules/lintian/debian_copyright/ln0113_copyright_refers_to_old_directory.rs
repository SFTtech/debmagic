use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::installed_copyright;

pub struct CopyrightRefersToOldDirectory;

declare_rule! {
    /// The copyright file mentions `usr/share/doc/copyright` or `usr/doc/copyright`.
    ///
    /// Each phrase is its own Diagnostic. Extra and Location are empty.
    /// Udebs are not checked.
    CopyrightRefersToOldDirectory,
    code = "LN0113",
    tag = "copyright-refers-to-old-directory",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightRefersToOldDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let found = installed_copyright::inspect(files, &package);
        for _ in 0..found.old_directory {
            ctx.diagnostic(Diagnostic::error(""));
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
                TestFile::new("See /usr/share/doc/example/copyright"),
            ),
        ])];
        let fail = vec![case(vec![
            ("usr/share/doc/example", TestFile::directory()),
            (
                "usr/share/doc/example/copyright",
                TestFile::new("installed in usr/doc/copyright"),
            ),
        ])];
        BinaryPackageTester::new(CopyrightRefersToOldDirectory, pass, fail).test_and_snapshot();
    }
}
