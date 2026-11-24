use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::super::changelog_doc::{doc_files, native_package};

pub struct WrongNameForUpstreamChangelog;

declare_rule! {
    /// A non-native package has no upstream text changelog under its standard
    /// name (`changelog`, `changelog.gz`, `changelog.html`, optionally
    /// gzipped), but has a file in its doc directory whose name starts with
    /// `change` (any case) and does not contain `debian`. The first such file
    /// is the Location, with an empty extra.
    ///
    /// Native means the version in `Source (version)`, else `Version`, has no
    /// hyphen. Only the direct files of `/usr/share/doc/<package>/` count, and
    /// a symlinked doc directory is skipped. Udebs are not checked.
    WrongNameForUpstreamChangelog,
    code = "LN1205",
    tag = "wrong-name-for-upstream-changelog",
    default_selected = true,
    experimental = false,
}

static STANDARD_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^changelog(?:\.html)?(?:\.gz)?$").expect("valid regex"));
static CHANGE_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^change").expect("valid regex"));
static DEBIAN_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)debian").expect("valid regex"));

impl BinaryPackageRule for WrongNameForUpstreamChangelog {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = doc_files(ctx) else {
            return;
        };
        if native_package(ctx) {
            return;
        }
        if files.iter().any(|(name, _)| STANDARD_NAME.is_match(name)) {
            return;
        }
        let Some((_, path)) = files
            .into_iter()
            .find(|(name, _)| CHANGE_NAME.is_match(name) && !DEBIAN_NAME.is_match(name))
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
            // The standard names.
            files(&["usr/share/doc/example/changelog.gz"]).field("Version", "1.0-1"),
            files(&["usr/share/doc/example/changelog.html.gz"]).field("Version", "1.0-1"),
            // Native: not judged here.
            files(&["usr/share/doc/example/Changes"]).field("Version", "1.0"),
            // Mentions debian, so it is not an upstream changelog.
            files(&["usr/share/doc/example/changelog.Debian.gz"]).field("Version", "1.0-1"),
            // Does not start with `change`.
            files(&["usr/share/doc/example/NEWS"]).field("Version", "1.0-1"),
            // Only direct files of the doc directory count.
            files(&["usr/share/doc/example/sub/Changes"]).field("Version", "1.0-1"),
            // Udebs are not checked.
            files(&["usr/share/doc/example/Changes"])
                .field("Version", "1.0-1")
                .udeb(),
        ];
        let fail = vec![
            files(&["usr/share/doc/example/Changes"]).field("Version", "1.0-1"),
            files(&[
                "usr/share/doc/example/changes.txt",
                "usr/share/doc/example/NEWS",
            ])
            .field("Version", "1.0-1"),
        ];
        BinaryPackageTester::new(WrongNameForUpstreamChangelog, pass, fail).test_and_snapshot();
    }
}
