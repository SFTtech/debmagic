use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::shadow::shadowed;

pub struct FileInRootAndUsr;

declare_rule! {
    /// A root-level file or directory is also installed under `usr/`.
    ///
    /// Data files under `bin/`, `sbin/`, `lib/`, `lib32/`, `libo32/`, `libx32/`
    /// and `lib64/` are compared with `usr/` at the same path. Both directories
    /// is fine. Otherwise the root entry is reported, unless its path is a
    /// shared library or static archive (`lib*.so*` or `lib*.a`), which is
    /// `library-in-root-and-usr`. Extra is `already in: usr/<path>`; the root
    /// path is the Location, with a trailing slash for directories. Udebs are
    /// checked too.
    FileInRootAndUsr,
    code = "LN0780",
    tag = "file-in-root-and-usr",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for FileInRootAndUsr {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for item in shadowed(ctx).into_iter().filter(|item| !item.library) {
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
            tree(vec![("bin/script", TestFile::new("x"))]),
            tree(vec![("usr/bin/script", TestFile::new("x"))]),
            tree(vec![
                ("bin/sub", TestFile::directory()),
                ("usr/bin/sub", TestFile::directory()),
            ]),
            tree(vec![
                ("libexec/foo", TestFile::new("x")),
                ("usr/libexec/foo", TestFile::new("x")),
            ]),
            tree(vec![
                ("lib/libbasic.so.2", TestFile::new("x")),
                ("usr/lib/libbasic.so.2", TestFile::new("x")),
            ]),
            tree(vec![
                ("bin/script", TestFile::new("x")),
                ("usr/bin/other", TestFile::new("x")),
            ]),
        ];
        let fail = vec![
            tree(vec![
                ("bin/script", TestFile::new("x")),
                ("usr/bin/script", TestFile::new("x")),
            ]),
            tree(vec![
                ("sbin/tool", TestFile::new("x")),
                ("usr/sbin/tool", TestFile::new("x")),
            ]),
            tree(vec![
                ("bin/sub", TestFile::new("x")),
                ("usr/bin/sub", TestFile::directory()),
            ]),
            tree(vec![
                ("lib64/data.txt", TestFile::new("x")),
                ("usr/lib64/data.txt", TestFile::new("x")),
            ]),
            tree(vec![
                ("bin/script", TestFile::new("x")),
                ("usr/bin/script", TestFile::new("x")),
            ])
            .udeb(),
        ];
        BinaryPackageTester::new(FileInRootAndUsr, pass, fail).test_and_snapshot();
    }
}
