use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

pub struct NodejsModuleNotDeclared;

declare_rule! {
    /// A nodejs module directory is installed without a matching `Provides`.
    ///
    /// `package.json` directly in `usr/share/nodejs/<module>`,
    /// `usr/lib/nodejs/<module>`, or `usr/lib/<triplet>/nodejs/<module>`,
    /// including `@scope/name`. Nested modules are not this Tag, and a
    /// directory name that differs from the JSON `name` is not either.
    /// The JSON object needs a non-empty string `name`. The declared name is
    /// `node-` plus that name, lower-cased, with `/`, `_`, and `@` turned
    /// into `-` and repeated dashes collapsed. `Provides` together with the
    /// package's own `name (= version)` must imply that name. Packages whose
    /// names end in `-dbg` are not checked. Extra is the declared name. The
    /// path is the Location. Udebs are checked too.
    NodejsModuleNotDeclared,
    code = "LN0451",
    tag = "nodejs-module-not-declared",
    default_selected = true,
    experimental = false,
}

static PACKAGE_JSON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/(?:share|lib(?:/[^/]+)?)/nodejs/([^\@/]+|@[^/]+/[^/]+)(.*/)package\.json$")
        .expect("valid regex")
});

impl BinaryPackageRule for NodejsModuleNotDeclared {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name().ends_with("-dbg") {
            return;
        }
        let fields = ctx.installation_control().fields();
        let version = fields.get("Version").unwrap_or("");
        let declared = if version.is_empty() {
            ctx.package_name().to_string()
        } else {
            format!("{} (= {version})", ctx.package_name())
        };
        let provides = fields
            .relation("Provides")
            .unwrap_or_default()
            .and(&PackageRelation::parse(&declared));
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                continue;
            }
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            let Some(captures) = PACKAGE_JSON.captures(path) else {
                continue;
            };
            let dirname = captures.get(1).map(|m| m.as_str()).unwrap_or("");
            let subpath = captures.get(2).map(|m| m.as_str()).unwrap_or("");
            if subpath != "/" {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            let Some(node_name) = declared_module(&bytes, dirname) else {
                continue;
            };
            if provides.satisfies(&node_name) {
                continue;
            }
            hits.push((entry.path().to_path_buf(), node_name));
        }
        hits.sort_by(|left, right| left.0.cmp(&right.0));
        for (path, node_name) in hits {
            ctx.diagnostic(Diagnostic::warn(node_name).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// The `node-…` name when this `package.json` is the module root and its
/// `name` matches the directory. `None` when the file is not this Tag.
fn declared_module(bytes: &[u8], dirname: &str) -> Option<String> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let name = value.get("name").and_then(Value::as_str)?;
    if name.is_empty() || name != dirname {
        return None;
    }
    Some(node_package_name(name))
}

fn node_package_name(name: &str) -> String {
    let mut raw = String::from("node-");
    for ch in name.to_lowercase().chars() {
        raw.push(match ch {
            '/' | '_' | '@' => '-',
            other => other,
        });
    }
    let mut declared = String::new();
    let mut previous_dash = false;
    for ch in raw.chars() {
        if ch == '-' {
            if previous_dash {
                continue;
            }
            previous_dash = true;
        } else {
            previous_dash = false;
        }
        declared.push(ch);
    }
    declared
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn module(package: &str, path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(body))]).package(package)
    }

    #[test]
    fn test() {
        let body = "{\"name\":\"mongodb\",\"version\":\"3.5.2\"}\n";
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            module(
                "libjs-mongodb",
                "usr/share/nodejs/mongodb/package.json",
                body,
            )
            .field("Provides", "node-mongodb"),
            module(
                "node-mongodb",
                "usr/share/nodejs/mongodb/package.json",
                body,
            )
            .field("Version", "3.5.2-1"),
            module(
                "node-foo-dbg",
                "usr/share/nodejs/foo/package.json",
                "{\"name\":\"foo\"}\n",
            ),
            module(
                "libjs-mongodb",
                "usr/share/nodejs/mongodb/node_modules/bson/package.json",
                "{\"name\":\"bson\"}\n",
            ),
            module("libjs-mongodb", "usr/share/nodejs/other/package.json", body),
            module(
                "libjs-mongodb",
                "usr/share/nodejs/mongodb/package.json",
                "not json\n",
            ),
        ];
        let fail = vec![
            module(
                "libjs-mongodb",
                "usr/share/nodejs/mongodb/package.json",
                body,
            ),
            module(
                "node-foo",
                "usr/share/nodejs/@foo/bar/package.json",
                "{\"name\":\"@foo/bar\",\"version\":\"1.2.0\"}\n",
            ),
            module(
                "libjs-bson",
                "usr/lib/nodejs/bson/package.json",
                "{\"name\":\"bson\"}\n",
            )
            .field("Provides", "node-bson | node-other"),
            module(
                "libjs-mongodb",
                "usr/lib/x86_64-linux-gnu/nodejs/mongodb/package.json",
                body,
            )
            .udeb(),
        ];
        BinaryPackageTester::new(NodejsModuleNotDeclared, pass, fail).test_and_snapshot();
    }

    #[test]
    fn node_names_collapse_separators() {
        assert_eq!(node_package_name("MongoDB"), "node-mongodb");
        assert_eq!(node_package_name("@foo/bar"), "node-foo-bar");
        assert_eq!(node_package_name("foo_bar"), "node-foo-bar");
    }
}
