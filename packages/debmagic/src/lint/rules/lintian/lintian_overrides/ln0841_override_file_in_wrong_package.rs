use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct OverrideFileInWrongPackage;

declare_rule! {
    /// A Lintian override is installed under another package's name.
    ///
    /// A Data file `usr/share/lintian/overrides/<other>/…` whose path is not
    /// already reported as `override-file-in-wrong-location` (that is, not
    /// under this package's own override directory). Extra is the name
    /// between `overrides/` and the last `/`, as lintian captures it, so a
    /// deeper path keeps its inner slashes. The path is the Location. Udebs
    /// are checked too.
    OverrideFileInWrongPackage,
    code = "LN0841",
    tag = "override-file-in-wrong-package",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for OverrideFileInWrongPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = regex::escape(ctx.package_name());
        let own = Regex::new(&format!("^usr/share/lintian/overrides/{package}/.+$"))
            .expect("valid regex");
        let other = Regex::new(r"^usr/share/lintian/overrides/(.+)/.+$").expect("valid regex");
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut found: Vec<(std::path::PathBuf, String)> = Vec::new();
        for entry in files.entries() {
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            if own.is_match(path) {
                continue;
            }
            let Some(captures) = other.captures(path) else {
                continue;
            };
            found.push((entry.path().to_path_buf(), captures[1].to_string()));
        }
        found.sort();
        for (path, expected) in found {
            ctx.diagnostic(Diagnostic::error(expected).with_location(Location {
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

    fn placed(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("tag\n"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            placed("usr/share/lintian/overrides/example"),
            placed("usr/share/doc/other/override.lintian"),
            placed("usr/share/lintian/overrides/example/some-override"),
            placed("usr/share/lintian/overrides/other"),
            placed("usr/share/doc/example/override.lintian"),
            BinaryPackageCase::from(vec![(
                "usr/share/lintian/overrides/other".to_string(),
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            placed("usr/share/lintian/overrides/other/some-override"),
            placed("usr/share/lintian/overrides/other/x.gz"),
            placed("usr/share/lintian/overrides/a/b/c"),
            placed("usr/share/lintian/overrides/other/sub/file"),
            BinaryPackageCase::from(vec![(
                "usr/share/lintian/overrides/other/sub".to_string(),
                TestFile::directory(),
            )]),
            placed("usr/share/lintian/overrides/other/x").udeb(),
        ];
        BinaryPackageTester::new(OverrideFileInWrongPackage, pass, fail).test_and_snapshot();
    }
}
