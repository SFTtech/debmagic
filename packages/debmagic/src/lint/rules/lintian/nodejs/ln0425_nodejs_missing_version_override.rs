use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NodejsMissingVersionOverride;

declare_rule! {
    /// A shipped `package.json` still has a `0.0.0-dev` version.
    ///
    /// The file sits under `usr/share/nodejs`, `usr/lib/nodejs`, or
    /// `usr/lib/<triplet>/nodejs`, including a scoped `@scope/name` directory.
    /// The JSON object needs a non-empty string `name`. Extra is the name and
    /// the version. The path is the Location. Udebs are checked too.
    NodejsMissingVersionOverride,
    code = "LN0425",
    tag = "nodejs-missing-version-override",
    default_selected = true,
    experimental = false,
}

static PACKAGE_JSON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/(?:share|lib(?:/[^/]+)?)/nodejs/(?:[^\@/]+|@[^/]+/[^/]+).*/package\.json$")
        .expect("valid regex")
});

impl BinaryPackageRule for NodejsMissingVersionOverride {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
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
            if !PACKAGE_JSON.is_match(path) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            let Some(message) = dev_version(&bytes) else {
                continue;
            };
            hits.push((entry.path().to_path_buf(), message));
        }
        hits.sort_by(|left, right| left.0.cmp(&right.0));
        for (path, message) in hits {
            ctx.diagnostic(Diagnostic::error(message).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn dev_version(bytes: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let name = value.get("name").and_then(Value::as_str)?;
    if name.is_empty() {
        return None;
    }
    let version = value.get("version").and_then(Value::as_str).unwrap_or("");
    if version.is_empty() || !version.starts_with("0.0.0-dev") {
        return None;
    }
    Some(format!("{name} {version}"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package_json(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            package_json(
                "usr/share/nodejs/mongodb/package.json",
                "{\"name\":\"mongodb\",\"version\":\"1.2.3\"}\n",
            ),
            package_json(
                "usr/share/nodejs/package.json",
                "{\"name\":\"root\",\"version\":\"0.0.0-dev\"}\n",
            ),
            package_json("usr/share/nodejs/mongodb/package.json", "not json\n"),
            package_json(
                "usr/share/nodejs/mongodb/package.json",
                "{\"version\":\"0.0.0-dev\"}\n",
            ),
        ];
        let fail = vec![
            package_json(
                "usr/share/nodejs/mongodb/package.json",
                "{\"name\":\"mongodb\",\"version\":\"0.0.0-development\"}\n",
            ),
            package_json(
                "usr/share/nodejs/@foo/bar/package.json",
                "{\"name\":\"@foo/bar\",\"version\":\"0.0.0-dev\"}\n",
            ),
            package_json(
                "usr/lib/nodejs/bson/node_modules/x/package.json",
                "{\"name\":\"bson\",\"version\":\"0.0.0-dev.1\"}\n",
            ),
        ];
        BinaryPackageTester::new(NodejsMissingVersionOverride, pass, fail).test_and_snapshot();
    }
}
