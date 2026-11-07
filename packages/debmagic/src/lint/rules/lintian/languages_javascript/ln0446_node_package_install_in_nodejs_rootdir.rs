use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NodePackageInstallInNodejsRootdir;

declare_rule! {
    /// `package.json` or a `*.js` file sits directly in a nodejs root directory.
    ///
    /// `usr/share/nodejs`, `usr/lib/nodejs`, and `usr/lib/<triplet>/nodejs`.
    /// Directories are ignored. Packages whose names end in `-dbg` are not
    /// checked. Extra is empty. Udebs are checked too.
    NodePackageInstallInNodejsRootdir,
    code = "LN0446",
    tag = "node-package-install-in-nodejs-rootdir",
    default_selected = true,
    experimental = false,
}

static ROOT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/(?:share|lib(?:/[^/]+)?)/nodejs/(?:package\.json|[^/]*\.js)$")
        .expect("valid regex")
});

impl BinaryPackageRule for NodePackageInstallInNodejsRootdir {
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
                ROOT.is_match(path).then(|| entry.path().to_path_buf())
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
            packaged("node-foo", "usr/share/nodejs/foo/package.json"),
            packaged("node-foo-dbg", "usr/share/nodejs/test.js"),
            packaged("node-foo", "usr/share/nodejs/readme.txt"),
            BinaryPackageCase::from(vec![(
                "usr/share/nodejs/package.json",
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            packaged("node-foo", "usr/share/nodejs/test.js"),
            packaged("node-foo", "usr/share/nodejs/package.json"),
            packaged("node-foo", "usr/lib/x86_64-linux-gnu/nodejs/index.js"),
            packaged("node-foo", "usr/lib/nodejs/test.js").udeb(),
        ];
        BinaryPackageTester::new(NodePackageInstallInNodejsRootdir, pass, fail).test_and_snapshot();
    }
}
