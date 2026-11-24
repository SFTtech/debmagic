use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct NodejsModuleInstalledInBadDirectory;

declare_rule! {
    /// A Node.js module is installed in a directory whose name does not match
    /// the module's `name` in its `package.json`.
    ///
    /// Reads the Data files `package.json` that sit at the top of a module
    /// directly under a nodejs root (`usr/share/nodejs/`, `usr/lib/nodejs/`, or
    /// `usr/lib/<multiarch>/nodejs/`). The directory name is the first path
    /// component after the root, or `@scope/name` for a scoped module. When the
    /// file's JSON `name` is a non-empty string different from that directory,
    /// the Rule reports with extra `<name> <directory>` and the path as the
    /// Location. Only regular files and hardlinks are read: a symlinked
    /// `package.json` is not followed, and an unparsable file or one with no
    /// `name` is skipped. Packages named `*-dbg` are skipped. Udebs are checked
    /// too. Other nodejs Tags are not part of this Rule.
    NodejsModuleInstalledInBadDirectory,
    code = "LN0930",
    tag = "nodejs-module-installed-in-bad-directory",
    default_selected = true,
    experimental = false,
}

static TOP_PACKAGE_JSON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/(?:share|lib(?:/[^/]+)?)/nodejs/([^@/]+|@[^/]+/[^/]+)/package\.json$")
        .expect("valid regex")
});

impl BinaryPackageRule for NodejsModuleInstalledInBadDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name().ends_with("-dbg") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits = bad_directories(files);
        for (path, name, directory) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("{name} {directory}")).with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

/// The top-level `package.json` files whose module `name` differs from the
/// directory they sit in: the path, the module name, and the directory.
fn bad_directories(files: &FileTree) -> Vec<(PathBuf, String, String)> {
    files
        .entries()
        .iter()
        .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
        .filter_map(|entry| {
            let path = entry.path().to_str()?;
            let captures = TOP_PACKAGE_JSON.captures(path)?;
            let directory = captures.get(1)?.as_str().to_string();
            let name = module_name(files, entry)?;
            (name != directory).then(|| (entry.path().to_path_buf(), name, directory))
        })
        .collect()
}

/// The non-empty string `name` of a `package.json`, when it parses as JSON.
fn module_name(files: &FileTree, entry: &Entry) -> Option<String> {
    let bytes = files.read(entry).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value
        .get("name")?
        .as_str()
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn json(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Matching directory.
            json("usr/share/nodejs/bson/package.json", r#"{"name": "bson"}"#),
            json("usr/lib/nodejs/bson/package.json", r#"{"name": "bson"}"#),
            json(
                "usr/share/nodejs/@foo/bar/package.json",
                r#"{"name": "@foo/bar"}"#,
            ),
            // Not a top-level module package.json.
            json(
                "usr/share/nodejs/bson/node_modules/x/package.json",
                r#"{"name": "other"}"#,
            ),
            // Unparsable, or no usable name.
            json("usr/share/nodejs/badname/package.json", "{not json"),
            json(
                "usr/share/nodejs/badname/package.json",
                r#"{"version": "1"}"#,
            ),
            json("usr/share/nodejs/badname/package.json", r#"{"name": ""}"#),
            // Not under a nodejs root.
            json("usr/share/doc/badname/package.json", r#"{"name": "bson"}"#),
            // Not a package.json file name.
            json(
                "usr/share/nodejs/badname/package.lock",
                r#"{"name": "bson"}"#,
            ),
            json("usr/share/nodejs/badname/index.js", r#"{"name": "bson"}"#),
            // A -dbg package is skipped.
            json(
                "usr/share/nodejs/badname/package.json",
                r#"{"name": "bson"}"#,
            )
            .package("bson-dbg"),
            // Udeb-free check of the non-regular case: a symlink is not read.
            BinaryPackageCase::from(vec![(
                "usr/share/nodejs/badname/package.json",
                TestFile::symlink("/elsewhere"),
            )]),
        ];
        let fail = vec![
            json(
                "usr/share/nodejs/badname/package.json",
                r#"{"name": "bson"}"#,
            ),
            json("usr/lib/nodejs/badname/package.json", r#"{"name": "bson"}"#),
            json(
                "usr/lib/x86_64-linux-gnu/nodejs/badname/package.json",
                r#"{"name": "bson"}"#,
            ),
            json(
                "usr/share/nodejs/@foo/bar/package.json",
                r#"{"name": "@foo/baz"}"#,
            ),
            json(
                "usr/share/nodejs/badname/package.json",
                r#"{"name": "bson"}"#,
            )
            .udeb(),
            BinaryPackageCase::from(vec![(
                "usr/share/nodejs/badname/package.json",
                TestFile::new(r#"{"name": "bson"}"#).hardlink(),
            )]),
        ];
        BinaryPackageTester::new(NodejsModuleInstalledInBadDirectory, pass, fail)
            .test_and_snapshot();
    }
}
