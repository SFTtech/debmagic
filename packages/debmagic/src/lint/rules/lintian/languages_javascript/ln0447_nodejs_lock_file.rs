use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NodejsLockFile;

declare_rule! {
    /// A `package-lock.json` or `yarn.lock` is installed under a nodejs module.
    ///
    /// The lock file is inside a module directory of `usr/share/nodejs`,
    /// `usr/lib/nodejs`, or `usr/lib/<triplet>/nodejs`, not the root itself.
    /// Directories are ignored. Packages whose names end in `-dbg` are not
    /// checked. Extra is empty. Udebs are checked too.
    NodejsLockFile,
    code = "LN0447",
    tag = "nodejs-lock-file",
    default_selected = true,
    experimental = false,
}

static LOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/(?:share|lib(?:/[^/]+)?)/nodejs/([^/]+)(.*/)(package-lock\.json|yarn\.lock)$")
        .expect("valid regex")
});

impl BinaryPackageRule for NodejsLockFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name().ends_with("-dbg") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                LOCK.is_match(path).then(|| entry.path().to_path_buf())
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

    fn packaged(name: &str, path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("{}"))]).package(name)
    }

    #[test]
    fn test() {
        let pass = vec![
            packaged("node-foo", "usr/share/nodejs/package-lock.json"),
            packaged("node-foo", "usr/share/nodejs/foo/package.json"),
            packaged("node-foo-dbg", "usr/share/nodejs/foo/package-lock.json"),
        ];
        let fail = vec![
            packaged(
                "node-foo",
                "usr/share/nodejs/module-installs-lock-file/yarn.lock",
            ),
            packaged(
                "node-foo",
                "usr/share/nodejs/module-installs-lock-file/package-lock.json",
            ),
            packaged("node-foo", "usr/lib/x86_64-linux-gnu/nodejs/foo/yarn.lock").udeb(),
        ];
        BinaryPackageTester::new(NodejsLockFile, pass, fail).test_and_snapshot();
    }
}
