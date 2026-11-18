use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::shadow::shadowed;

pub struct LibraryInRootAndUsr;

declare_rule! {
    /// A root-level shared library or static archive is also under `usr/`.
    ///
    /// Data files under `bin/`, `sbin/`, `lib/`, `lib32/`, `libo32/`, `libx32/`
    /// and `lib64/` whose path matches `lib*.so*` or `lib*.a` are compared with
    /// `usr/` at the same path. Both directories is fine. Otherwise the root
    /// entry is reported. Extra is `already in: usr/<path>`; the root path is
    /// the Location, with a trailing slash for directories. Udebs are checked
    /// too.
    LibraryInRootAndUsr,
    code = "LN0781",
    tag = "library-in-root-and-usr",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LibraryInRootAndUsr {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for item in shadowed(ctx).into_iter().filter(|item| item.library) {
            ctx.diagnostic(
                Diagnostic::error(format!("already in: {}", item.usr.display())).with_location(
                    Location {
                        path: item.location,
                        line: None,
                        column: None,
                    },
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            tree(vec![("lib/libbasic.so.2", TestFile::new("x"))]),
            tree(vec![
                ("lib/libfoo.txt", TestFile::new("x")),
                ("usr/lib/libfoo.txt", TestFile::new("x")),
            ]),
            tree(vec![
                ("lib/libfoo.so", TestFile::directory()),
                ("usr/lib/libfoo.so", TestFile::directory()),
            ]),
            tree(vec![
                ("bin/libfoo.so", TestFile::new("x")),
                ("usr/bin/libfoo.so", TestFile::new("x")),
            ]),
            tree(vec![
                ("libexec/libfoo.so", TestFile::new("x")),
                ("usr/libexec/libfoo.so", TestFile::new("x")),
            ]),
        ];
        let fail = vec![
            tree(vec![
                ("lib/libbasic.so.2", TestFile::new("x")),
                ("usr/lib/libbasic.so.2", TestFile::new("x")),
            ]),
            tree(vec![
                ("lib64/libstatic.a", TestFile::new("x")),
                ("usr/lib64/libstatic.a", TestFile::new("x")),
            ]),
            tree(vec![
                ("lib/x86_64-linux-gnu/libbaz.so.1", TestFile::new("x")),
                ("usr/lib/x86_64-linux-gnu/libbaz.so.1", TestFile::new("x")),
            ]),
            tree(vec![
                ("lib/libfoo.so.1", TestFile::symlink("libfoo.so.1.0")),
                ("usr/lib/libfoo.so.1", TestFile::new("x")),
            ]),
            tree(vec![
                ("lib/libbasic.so.2", TestFile::new("x")),
                ("usr/lib/libbasic.so.2", TestFile::new("x")),
            ])
            .udeb(),
        ];
        BinaryPackageTester::new(LibraryInRootAndUsr, pass, fail).test_and_snapshot();
    }
}
