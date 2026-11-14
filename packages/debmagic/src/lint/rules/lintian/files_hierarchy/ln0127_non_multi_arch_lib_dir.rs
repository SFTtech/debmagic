use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NonMultiArchLibDir;

declare_rule! {
    /// A direct `usr/` directory is `lib64`, `lib32`, or `libx32`.
    ///
    /// Source `glibc` and `eglibc`, and a package whose name starts with
    /// `lib64`, `lib32`, or `libx32`, are exempt. Udebs are not checked.
    /// Extra is empty; the directory, with a trailing slash, is the Location.
    NonMultiArchLibDir,
    code = "LN0127",
    tag = "non-multi-arch-lib-dir",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NonMultiArchLibDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let source = ctx.source_name();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() == EntryKind::Directory)
            .filter_map(|entry| {
                let name = entry.path().to_str()?;
                let suffix = name.strip_prefix("usr/lib")?;
                if !matches!(suffix, "64" | "32" | "x32") {
                    return None;
                }
                if source == "glibc"
                    || source == "eglibc"
                    || package.starts_with(&format!("lib{suffix}"))
                {
                    return None;
                }
                Some(PathBuf::from(format!("usr/lib{suffix}/")))
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

    fn dirs(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| ((*path).to_string(), TestFile::directory()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            dirs(&["usr/lib"]),
            dirs(&["usr/lib64"]).field("Source", "glibc"),
            dirs(&["usr/lib32"]).field("Source", "eglibc (2.13-1)"),
            dirs(&["usr/lib64"]).package("lib64c"),
            dirs(&["lib64"]),
            dirs(&["usr/lib64"]).udeb(),
        ];
        let fail = vec![
            dirs(&["usr/lib64"]),
            dirs(&["usr/lib32"]),
            dirs(&["usr/libx32"]),
        ];
        BinaryPackageTester::new(NonMultiArchLibDir, pass, fail).test_and_snapshot();
    }
}
