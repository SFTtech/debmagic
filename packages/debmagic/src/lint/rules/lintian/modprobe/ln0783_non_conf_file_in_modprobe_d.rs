use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::scope::in_scope;

pub struct NonConfFileInModprobeD;

declare_rule! {
    /// A file below `usr/lib/` or `etc/` `modprobe.d` or `modules-load.d` does
    /// not end in `.conf`.
    ///
    /// Every non-directory Data files entry in those four directories, nested
    /// or not, whose path does not end in `.conf` is reported, because modprobe
    /// ignores it. Symlinks and hardlinks count as files. Extra is empty; the
    /// path is the Location. Udebs are checked too.
    NonConfFileInModprobeD,
    code = "LN0783",
    tag = "non-conf-file-in-modprobe.d",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NonConfFileInModprobeD {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| !matches!(entry.kind(), EntryKind::Directory))
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                (in_scope(path) && !path.ends_with(".conf")).then(|| entry.path().to_path_buf())
            })
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
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
            tree(vec![("etc/modprobe.d/ok.conf", TestFile::new("x"))]),
            tree(vec![(
                "usr/lib/modprobe.d/nested/install.conf",
                TestFile::new("x"),
            )]),
            tree(vec![("etc/modprobe.d/sub.d", TestFile::directory())]),
            tree(vec![(
                "etc/modprobe.d/link.conf",
                TestFile::symlink("ok.conf"),
            )]),
            tree(vec![("etc/other/notes.txt", TestFile::new("x"))]),
            tree(vec![("usr/share/modprobe.d/notes", TestFile::new("x"))]),
        ];
        let fail = vec![
            tree(vec![("usr/lib/modprobe.d/--lzma", TestFile::new("x"))]),
            tree(vec![("etc/modules-load.d/readme", TestFile::new("x"))]),
            tree(vec![("etc/modprobe.d/nested/x", TestFile::new("x"))]),
            tree(vec![("etc/modprobe.d/link", TestFile::symlink("ok.conf"))]),
            tree(vec![("etc/modprobe.d/foo.conf.bak", TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(NonConfFileInModprobeD, pass, fail).test_and_snapshot();
    }
}
