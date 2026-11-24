use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct Apache2UnparsableDependency;

declare_rule! {
    /// An Apache `# Depends:` or `# Conflicts:` line names a token that is not
    /// a module name.
    ///
    /// Read from `etc/apache2/{conf,site}-available/`, `etc/apache2/conf.d/`,
    /// and `etc/apache2/mods-available/*.{load,conf}`. A token with a character
    /// other than a word character or `.`, a token starting with `mod_`, or a
    /// token containing `.conf` or `.load` is reported. Extra is that token.
    /// The Location is the comment line. The web server packages are exempt.
    Apache2UnparsableDependency,
    code = "LN0391",
    tag = "apache2-unparsable-dependency",
    default_selected = true,
    experimental = false,
}

static FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^#\s*(Depends|Conflicts):\s+(.*?)\s*$").expect("valid regex"));
static ODD_CHARACTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^\w.]").expect("valid regex"));
static MOD_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^mod_").expect("valid regex"));
static FILE_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:conf|load)").expect("valid regex"));
static SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\n\s]+").expect("valid regex"));

impl BinaryPackageRule for Apache2UnparsableDependency {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if scan::is_web_server(ctx.package_name()) {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                    && entry.path().to_str().is_some_and(scan::inspect_conf)
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        let mut hits = Vec::new();
        for path in paths {
            let Some(entry) = files.get(&path) else {
                continue;
            };
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            for (index, line) in text.lines().enumerate() {
                let Some(captures) = FIELD.captures(line) else {
                    continue;
                };
                let value = captures.get(2).map(|m| m.as_str()).unwrap_or("");
                let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
                for dep in SPLIT.split(value) {
                    if dep.is_empty() {
                        continue;
                    }
                    if ODD_CHARACTER.is_match(dep)
                        || MOD_PREFIX.is_match(dep)
                        || FILE_SUFFIX.is_match(dep)
                    {
                        hits.push((path.clone(), line_number, dep.to_string()));
                    }
                }
            }
        }
        for (path, line, dep) in hits {
            ctx.diagnostic(Diagnostic::warn(dep).with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            file(
                "etc/apache2/conf-available/foo.conf",
                "# Depends: foo bar.baz\n",
            ),
            file(
                "etc/apache2/mods-available/foo.load",
                "# Depends: rewrite\n",
            ),
            file("usr/share/doc/example/foo.conf", "# Depends: bar2.conf\n"),
            file(
                "etc/apache2/conf-available/foo.conf",
                "# Depends: bar2.conf\n",
            )
            .package("apache2-data"),
        ];
        let fail = vec![
            file(
                "etc/apache2/conf-available/phpmyfoo.conf",
                "# Depends: bar2.conf\n",
            ),
            file(
                "etc/apache2/mods-available/foo.load",
                "# Depends: mod_baz3 baz4.load\n",
            ),
            file("etc/apache2/conf.d/phpmyfoo.conf", "# Conflicts: foo|bar\n"),
        ];
        BinaryPackageTester::new(Apache2UnparsableDependency, pass, fail).test_and_snapshot();
    }
}
