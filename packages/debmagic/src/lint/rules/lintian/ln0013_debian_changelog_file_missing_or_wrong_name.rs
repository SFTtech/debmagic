use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::changelog_doc::{MissingChangelog, missing_changelog};

pub struct DebianChangelogFileMissingOrWrongName;

declare_rule! {
    /// The Binary package has an upstream changelog but no Debian changelog.
    ///
    /// A foreign non-native package (the version in `Source`, else `Version`,
    /// has a `-`, and the `Version` has no `-0.`) ships `changelog` or
    /// `changelog.gz` directly in `usr/share/doc/<pkg>` but neither
    /// `changelog.Debian` nor `changelog.Debian.gz`, in any letter case. The
    /// same package with no changelog at all is `no-changelog`. A symlinked
    /// doc directory is judged by other Tags. Udebs are not checked. Extra and
    /// Location are empty.
    DebianChangelogFileMissingOrWrongName,
    code = "LN0013",
    tag = "debian-changelog-file-missing-or-wrong-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebianChangelogFileMissingOrWrongName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if missing_changelog(ctx) == Some(MissingChangelog::DebianButUpstreamText) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    /// Without a `Version` the package is non-native and foreign.
    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            // Both changelogs.
            files(&[
                "usr/share/doc/example/changelog.gz",
                "usr/share/doc/example/changelog.Debian.gz",
            ]),
            // No changelog at all is `no-changelog`.
            files(&["usr/share/doc/example/copyright"]),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Version", "1.0-1"),
            // Wrong name in another case is another Tag.
            files(&[
                "usr/share/doc/example/changelog.gz",
                "usr/share/doc/example/changelog.debian.gz",
            ]),
            // HTML only is not a text changelog.
            files(&["usr/share/doc/example/changelog.html.gz"]),
            // A native package has no Debian changelog.
            files(&["usr/share/doc/example/changelog.gz"]).field("Version", "1.0"),
            // A `-0.x` version is not known to be foreign.
            files(&["usr/share/doc/example/changelog.gz"]).field("Version", "1.0-0.1"),
            // A symlinked doc directory is judged elsewhere.
            BinaryPackageCase::from(vec![("usr/share/doc/example", TestFile::symlink("other"))])
                .field("Version", "1.0-1"),
            files(&["usr/share/doc/example/changelog.gz"]).udeb(),
            // Only direct children of the doc directory count.
            files(&["usr/share/doc/example/sub/changelog.gz"]),
            // The version in Source decides nativeness.
            files(&["usr/share/doc/example/changelog.gz"])
                .field("Source", "other (1.0)")
                .field("Version", "1.0-1"),
        ];
        let fail = vec![
            files(&["usr/share/doc/example/changelog.gz"]),
            files(&["usr/share/doc/example/changelog"]),
            // Both text and HTML.
            files(&[
                "usr/share/doc/example/changelog.html.gz",
                "usr/share/doc/example/changelog.gz",
            ]),
            // A symlink counts as a file.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/changelog.gz",
                TestFile::symlink("../other/changelog.gz"),
            )])
            .field("Version", "1.0-1"),
            // A hyphen in Source's version makes it non-native despite Version.
            files(&["usr/share/doc/example/changelog.gz"])
                .field("Source", "other (1.0-1)")
                .field("Version", "1.0"),
        ];
        BinaryPackageTester::new(DebianChangelogFileMissingOrWrongName, pass, fail)
            .test_and_snapshot();
    }
}
