use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NodejsModuleInstalledInUsrLib;

declare_rule! {
    /// A file is installed under the old `usr/lib/nodejs` directory.
    ///
    /// Directories are ignored. Packages whose names end in `-dbg` are not
    /// checked. `usr/lib/<triplet>/nodejs` is a different directory. Extra is
    /// empty. Udebs are checked too.
    NodejsModuleInstalledInUsrLib,
    code = "LN0434",
    tag = "nodejs-module-installed-in-usr-lib",
    default_selected = true,
    experimental = false,
}

static OLD_ROOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/nodejs/.*").expect("valid regex"));

impl BinaryPackageRule for NodejsModuleInstalledInUsrLib {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name().ends_with("-dbg") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                OLD_ROOT.is_match(path).then(|| entry.path().to_path_buf())
            })
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

    fn packaged(name: &str, path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("x"))]).package(name)
    }

    #[test]
    fn test() {
        let pass = vec![
            packaged("node-foo", "usr/share/nodejs/foo/test.js"),
            packaged("node-foo", "usr/lib/x86_64-linux-gnu/nodejs/foo/test.js"),
            packaged(
                "node-foo-dbg",
                "usr/lib/nodejs/module-installed-in-usr-lib/test.js",
            ),
            BinaryPackageCase::from(vec![(
                "usr/lib/nodejs/module-installed-in-usr-lib",
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            packaged(
                "nodejs-module-installed-in-usr-lib",
                "usr/lib/nodejs/module-installed-in-usr-lib/test.js",
            ),
            packaged(
                "nodejs-module-installed-in-usr-lib",
                "usr/lib/nodejs/module-installed-in-usr-lib/package.json",
            ),
            packaged("node-foo", "usr/lib/nodejs/foo/test.js").udeb(),
        ];
        BinaryPackageTester::new(NodejsModuleInstalledInUsrLib, pass, fail).test_and_snapshot();
    }
}
