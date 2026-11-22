use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scope::in_scope;

pub struct FileInEtcModprobeD;

declare_rule! {
    /// A `.conf` file sits under `etc/modprobe.d` or `etc/modules-load.d`.
    ///
    /// Such files belong in `usr/lib/modprobe.d` or `usr/lib/modules-load.d`,
    /// with a `rm_conffile` in a `.maintscript` for the old one. Reported for
    /// every Data files entry that is not a directory, is below one of those two
    /// `etc` directories, and ends in `.conf`. Extra is empty; the Location is
    /// the file with no line. Experimental, so not selected by default. Udebs
    /// are checked too.
    FileInEtcModprobeD,
    code = "LN1424",
    tag = "file-in-etc-modprobe.d",
    default_selected = false,
    experimental = true,
}

impl BinaryPackageRule for FileInEtcModprobeD {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| !entry.is_dir())
            .map(|entry| entry.path())
            .filter(|path| {
                path.to_str().is_some_and(|name| {
                    name.starts_with("etc/") && in_scope(name) && name.ends_with(".conf")
                })
            })
            .map(PathBuf::from)
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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

    fn file(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("alias x y\n"))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            file("usr/lib/modprobe.d/test.conf"),
            file("usr/lib/modules-load.d/test.conf"),
            // Not a .conf file: another Tag.
            file("etc/modprobe.d/test.txt"),
            // Outside the directories.
            file("etc/test.conf"),
            // A directory is another Tag.
            BinaryPackageCase::from(vec![("etc/modprobe.d/sub", TestFile::directory())]),
        ];
        let fail = vec![
            file("etc/modprobe.d/test.conf"),
            file("etc/modules-load.d/test.conf"),
            file("etc/modprobe.d/test.conf").udeb(),
        ];
        BinaryPackageTester::new(FileInEtcModprobeD, pass, fail).test_and_snapshot();
    }
}
