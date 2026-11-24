use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsFileToUsrX11r6;

declare_rule! {
    /// A path is installed under `usr/X11R6`.
    ///
    /// Every entry under `usr/X11R6`, including that directory. Directory
    /// locations keep the trailing slash. Symlinks are allowed. Extra is
    /// empty. The path is the Location. Udebs are checked too.
    PackageInstallsFileToUsrX11r6,
    code = "LN0428",
    tag = "package-installs-file-to-usr-x11r6",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageInstallsFileToUsrX11r6 {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths = Vec::new();
        for entry in files.entries() {
            if matches!(entry.kind(), EntryKind::Symlink) {
                continue;
            }
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            let trimmed = path.trim_end_matches('/');
            if trimmed == "usr/X11R6" || trimmed.starts_with("usr/X11R6/") {
                let mut pointed = trimmed.to_string();
                if entry.kind() == EntryKind::Directory {
                    pointed.push('/');
                }
                paths.push(PathBuf::from(pointed));
            }
        }
        paths.sort();
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

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("usr/bin/xterm".to_string(), TestFile::new("bin\n"))]),
            BinaryPackageCase::from(vec![(
                "usr/X11R6/bin/tool".to_string(),
                TestFile::symlink("/usr/bin/tool"),
            )]),
        ];
        let fail = vec![BinaryPackageCase::from(vec![
            ("usr/X11R6/".to_string(), TestFile::directory()),
            ("usr/X11R6/bin/".to_string(), TestFile::directory()),
            ("usr/X11R6/bin/testxbin2".to_string(), TestFile::new("x\n")),
        ])];
        BinaryPackageTester::new(PackageInstallsFileToUsrX11r6, pass, fail).test_and_snapshot();
    }
}
