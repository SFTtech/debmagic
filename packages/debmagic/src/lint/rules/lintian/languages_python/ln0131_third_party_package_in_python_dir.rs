use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct ThirdPartyPackageInPythonDir;

declare_rule! {
    /// A path under `usr/lib/pythonX.Y/` is outside `site-packages` and `dist-packages`.
    ///
    /// Source packages named `python`, `pythonX.Y`, or
    /// `python[23]-(stdlib-extensions|profiler|old-doctools)` are exempt.
    /// Extra is empty; the path is the Location. Udebs are checked too.
    ThirdPartyPackageInPythonDir,
    code = "LN0131",
    tag = "third-party-package-in-python-dir",
    default_selected = true,
    experimental = false,
}

static PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/python\d\.\d/\S").expect("valid regex"));
static PACKAGES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/python\d\.\d/(?:site|dist)-packages/").expect("valid regex")
});
static ALLOWED_SOURCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:python(?:\d\.\d)?|python\d?-(?:stdlib-extensions|profiler|old-doctools))$")
        .expect("valid regex")
});

impl BinaryPackageRule for ThirdPartyPackageInPythonDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let source = source_name(ctx);
        if ALLOWED_SOURCE.is_match(&source) {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter_map(|entry| {
                let raw = entry.path().to_str()?;
                let name = if entry.kind() == EntryKind::Directory {
                    format!("{raw}/")
                } else {
                    raw.to_string()
                };
                (PATH.is_match(&name) && !PACKAGES.is_match(&name)).then_some(PathBuf::from(name))
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

fn source_name(ctx: &BinaryPackageContext<'_>) -> String {
    ctx.installation_control()
        .fields()
        .get("Source")
        .and_then(|value| value.split_whitespace().next())
        .filter(|name| !name.is_empty())
        .unwrap_or(ctx.package_name())
        .to_string()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/python3.11/dist-packages/foo.py".to_string(),
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/python3.11/foo.py".to_string(),
                TestFile::new("x"),
            )])
            .field("Source", "python3.11"),
        ];
        let fail = vec![BinaryPackageCase::from(vec![(
            "usr/lib/python2.7/bar".to_string(),
            TestFile::new("x"),
        )])];
        BinaryPackageTester::new(ThirdPartyPackageInPythonDir, pass, fail).test_and_snapshot();
    }
}
