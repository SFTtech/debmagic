use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct InitScriptIsNotAFile;

declare_rule! {
    /// An entry directly in `etc/init.d` is not a regular file or hardlink.
    ///
    /// Directories are skipped, as are `README`, `skeleton`, `rc` and `rcS`,
    /// and a symbolic link whose stored target is exactly `lib/init/upstart-job`.
    /// Symbolic links to ordinary files are reported too, since they are not
    /// files. The `initscripts` package is skipped. Extra is empty. The entry
    /// path is the Location. Udebs are checked too.
    InitScriptIsNotAFile,
    code = "LN0992",
    tag = "init-script-is-not-a-file",
    default_selected = true,
    experimental = false,
}

const SKIPPED: &[&str] = &["README", "skeleton", "rc", "rcS"];

impl BinaryPackageRule for InitScriptIsNotAFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name() == "initscripts" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.path().parent() == Some(Path::new("etc/init.d")))
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter(|entry| {
                let base = entry.path().file_name().and_then(|name| name.to_str());
                !base.is_some_and(|name| SKIPPED.contains(&name))
            })
            .filter(|entry| entry.link_target() != Some(Path::new("lib/init/upstart-job")))
            .filter(|entry| !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(at(path)));
        }
    }
}

fn at(path: PathBuf) -> Location {
    Location {
        path,
        line: None,
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            script("etc/init.d/good", TestFile::new("#!/bin/sh\n")),
            script("etc/init.d/README", TestFile::symlink("/usr/share/doc/x")),
            script(
                "etc/init.d/upstart",
                TestFile::symlink("lib/init/upstart-job"),
            ),
            script("etc/init.d/sub", TestFile::directory()),
            script("etc/init.d/hard", TestFile::new("#!/bin/sh\n").hardlink()),
            script("etc/init.d/linked", TestFile::symlink("/usr/share/foo")).package("initscripts"),
        ];
        let fail = vec![
            script("etc/init.d/linked", TestFile::symlink("/usr/share/foo")),
            script("etc/init.d/relative", TestFile::symlink("../default/other")),
            script(
                "etc/init.d/absolute",
                TestFile::symlink("/lib/init/upstart-job"),
            ),
            script("etc/init.d/udeb", TestFile::symlink("/usr/share/foo")).udeb(),
        ];
        BinaryPackageTester::new(InitScriptIsNotAFile, pass, fail).test_and_snapshot();
    }
}
