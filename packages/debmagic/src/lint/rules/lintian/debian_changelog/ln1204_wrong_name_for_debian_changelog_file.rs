use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::super::changelog_doc::{doc_files, native_package};

pub struct WrongNameForDebianChangelogFile;

declare_rule! {
    /// A non-native package has no `changelog.Debian` or `changelog.Debian.gz`
    /// in its doc directory, but has a `changelog.debian` (any case, `.gz`
    /// optional) there instead.
    ///
    /// Native means the version in `Source (version)`, else `Version`, has no
    /// hyphen. Only the direct files of `/usr/share/doc/<package>/` count, and
    /// a symlinked doc directory is skipped. The first such file is the
    /// Location, with an empty extra. Udebs are not checked.
    WrongNameForDebianChangelogFile,
    code = "LN1204",
    tag = "wrong-name-for-debian-changelog-file",
    default_selected = true,
    experimental = false,
}

static DEBIAN_CHANGELOG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^changelog\.Debian(?:\.gz)?$").expect("valid regex"));
static WRONG_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^changelog\.debian(?:\.gz)?$").expect("valid regex"));

impl BinaryPackageRule for WrongNameForDebianChangelogFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = doc_files(ctx) else {
            return;
        };
        if native_package(ctx) {
            return;
        }
        if files
            .iter()
            .any(|(name, _)| DEBIAN_CHANGELOG.is_match(name))
        {
            return;
        }
        let Some((_, path)) = files
            .into_iter()
            .find(|(name, _)| WRONG_NAME.is_match(name))
        else {
            return;
        };
        ctx.diagnostic(Diagnostic::error("").with_location(Location {
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
            files(&["usr/share/doc/example/changelog.Debian.gz"]).field("Version", "1.0-1"),
            files(&["usr/share/doc/example/changelog.Debian"]).field("Version", "1.0-1"),
            // Native: the Debian changelog name is not judged here.
            files(&["usr/share/doc/example/changelog.debian.gz"]).field("Version", "1.0"),
            // Not the wrong name.
            files(&["usr/share/doc/example/changelog.gz"]).field("Version", "1.0-1"),
            // Only direct files of the doc directory count.
            files(&["usr/share/doc/example/sub/changelog.debian.gz"]).field("Version", "1.0-1"),
            // Udebs are not checked.
            files(&["usr/share/doc/example/changelog.debian.gz"])
                .field("Version", "1.0-1")
                .udeb(),
        ];
        let fail = vec![
            files(&["usr/share/doc/example/changelog.debian.gz"]).field("Version", "1.0-1"),
            // The upstream changelog does not keep it from being reported.
            files(&[
                "usr/share/doc/example/changelog.gz",
                "usr/share/doc/example/changelog.debian",
            ])
            .field("Version", "1.0-1"),
        ];
        BinaryPackageTester::new(WrongNameForDebianChangelogFile, pass, fail).test_and_snapshot();
    }
}
