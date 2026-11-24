use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::changelog_doc::is_native;

pub struct WrongNameForChangelogOfNativePackage;

declare_rule! {
    /// A native package has no `changelog` or `changelog.gz` in its doc
    /// directory, but has a `changelog.debian.gz` there instead.
    ///
    /// Native means the version in `Source (version)`, else `Version`, has no
    /// hyphen. Only the direct files of `/usr/share/doc/<package>/` count, and
    /// a symlinked doc directory is skipped. The file is the Location, with an
    /// empty extra. Udebs are not checked.
    WrongNameForChangelogOfNativePackage,
    code = "LN1203",
    tag = "wrong-name-for-changelog-of-native-package",
    default_selected = true,
    experimental = false,
}

static NATIVE_CHANGELOG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^changelog(?:\.gz)?$").expect("valid regex"));
static WRONG_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^changelog\.debian\.gz$").expect("valid regex"));

impl BinaryPackageRule for WrongNameForChangelogOfNativePackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let fields = ctx.installation_control().fields();
        if !is_native(fields.get("Source"), fields.get("Version")) {
            return;
        }
        let doc_dir = format!("usr/share/doc/{package}");
        let prefix = format!("{doc_dir}/");
        let Some(files) = ctx.data_files() else {
            return;
        };
        if files
            .get(&doc_dir)
            .is_some_and(|entry| entry.kind() == EntryKind::Symlink)
        {
            return;
        }
        let direct: Vec<(String, PathBuf)> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter_map(|entry| {
                let name = entry.path().to_str()?.strip_prefix(&prefix)?;
                (!name.contains('/')).then(|| (name.to_string(), entry.path().to_path_buf()))
            })
            .collect();
        if direct
            .iter()
            .any(|(name, _)| NATIVE_CHANGELOG.is_match(name))
        {
            return;
        }
        let Some((_, path)) = direct
            .into_iter()
            .find(|(name, _)| WRONG_NAME.is_match(name))
        else {
            return;
        };
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
            path,
            line: None,
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            // The right name.
            files(&["usr/share/doc/example/changelog.gz"]).field("Version", "1.0"),
            // Both names: the right one is there.
            files(&[
                "usr/share/doc/example/changelog.gz",
                "usr/share/doc/example/changelog.debian.gz",
            ])
            .field("Version", "1.0"),
            // Foreign: the Debian changelog name is not judged here.
            files(&["usr/share/doc/example/changelog.debian.gz"]).field("Version", "1.0-1"),
            // Not the wrong name (no `.gz`).
            files(&["usr/share/doc/example/changelog.debian"]).field("Version", "1.0"),
            // Only direct files of the doc directory count.
            files(&["usr/share/doc/example/sub/changelog.debian.gz"]).field("Version", "1.0"),
            // Udebs are not checked.
            files(&["usr/share/doc/example/changelog.debian.gz"])
                .field("Version", "1.0")
                .udeb(),
        ];
        let fail = vec![
            files(&["usr/share/doc/example/changelog.debian.gz"]).field("Version", "1.0"),
            // Case-insensitive, and a hyphen in Source's version still makes it native.
            files(&["usr/share/doc/example/Changelog.Debian.gz"])
                .field("Source", "example (1.0)")
                .field("Version", "1.0-1"),
        ];
        BinaryPackageTester::new(WrongNameForChangelogOfNativePackage, pass, fail)
            .test_and_snapshot();
    }
}
