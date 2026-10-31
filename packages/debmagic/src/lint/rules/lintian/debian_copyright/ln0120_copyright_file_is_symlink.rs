use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::installed_copyright;

pub struct CopyrightFileIsSymlink;

declare_rule! {
    /// `usr/share/doc/<package>/copyright` is a symlink.
    ///
    /// Extra is empty; the symlink path is the Location. Udebs are not checked.
    CopyrightFileIsSymlink,
    code = "LN0120",
    tag = "copyright-file-is-symlink",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightFileIsSymlink {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let found = installed_copyright::inspect(files, &package);
        for path in found.symlink {
            ctx.diagnostic(
                Diagnostic::error("")
                    .with_severity(Severity::Error)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
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
            ("usr/share/doc/example/copyright", TestFile::new("plain")),
        ])];
        let fail = vec![case(vec![
            ("usr/share/doc/example", TestFile::directory()),
            (
                "usr/share/doc/example/copyright",
                TestFile::symlink("../other/copyright"),
            ),
        ])];
        BinaryPackageTester::new(CopyrightFileIsSymlink, pass, fail).test_and_snapshot();
    }
}
