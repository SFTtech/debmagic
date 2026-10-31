use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsDeprecatedPython2Path;

declare_rule! {
    /// The package installs a path under `usr/lib/python2`.
    ///
    /// The path must continue past `python2`. Directories are reported with a
    /// trailing slash. Extra is empty. Udebs are checked too.
    PackageInstallsDeprecatedPython2Path,
    code = "LN0129",
    tag = "package-installs-deprecated-python2-path",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageInstallsDeprecatedPython2Path {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
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
                name.strip_prefix("usr/lib/python2")
                    .is_some_and(|rest| !rest.is_empty())
                    .then_some(PathBuf::from(name))
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

    #[test]
    fn test() {
        let pass = vec![BinaryPackageCase::from(vec![
            (
                "usr/lib/python3/dist-packages/foo.py".to_string(),
                TestFile::new("x"),
            ),
            ("usr/lib/python2".to_string(), TestFile::new("x")),
        ])];
        let fail = vec![BinaryPackageCase::from(vec![
            ("usr/lib/python2.7".to_string(), TestFile::directory()),
            ("usr/lib/python2.7/bar".to_string(), TestFile::new("x")),
        ])];
        BinaryPackageTester::new(PackageInstallsDeprecatedPython2Path, pass, fail)
            .test_and_snapshot();
    }
}
