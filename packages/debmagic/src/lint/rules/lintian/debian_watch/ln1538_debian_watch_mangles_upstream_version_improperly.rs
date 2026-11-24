use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::file::{WatchFileHit, flag_watch_file};

pub struct DebianWatchManglesUpstreamVersionImproperly;

declare_rule! {
    /// The watch line mangles a prerelease with `dversionmangle`, but should
    /// use `uversionmangle`.
    ///
    /// Fires when the changelog upstream version contains `alpha`, `beta` or
    /// `rc` (any case), the last version field of the watch line is `debian`
    /// (or absent), a `[ud]?versionmangle` option names that word, and no
    /// `uversionmangle` option does. Extra is the joined watch line. The file
    /// and line are the Location. Info, not selected by default.
    DebianWatchManglesUpstreamVersionImproperly,
    code = "LN1538",
    tag = "debian-watch-mangles-upstream-version-improperly",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DebianWatchManglesUpstreamVersionImproperly {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_file(ctx, Severity::Info, |hit| {
            matches!(hit, WatchFileHit::UpstreamVersionMangledImproperly(_))
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn changelog(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * test\n\n -- Example <ex@example.com>  Tue, 31 Mar 2026 17:46:22 +0200\n"
        )
    }

    fn tree(version: &str, watch: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("debian/changelog", TestFile::from(changelog(version))),
            ("debian/watch", TestFile::from(watch)),
        ]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            // A final release has no prerelease word.
            tree(
                "1.0-1",
                "version=4\nopts=dversionmangle=s/~rc/-rc/ https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
            ),
            // The prerelease is mangled with uversionmangle.
            tree(
                "1.0~rc1-1",
                "version=4\nopts=uversionmangle=s/~rc/-rc/ https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
            ),
            // A dversionmangle that does not name the prerelease word.
            tree(
                "1.0~rc1-1",
                "version=4\nopts=dversionmangle=s/\\+dfsg$// https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
            ),
            // A version field other than `debian` keeps the prerelease out of this check.
            tree(
                "1.0~rc1-1",
                "version=4\nopts=dversionmangle=s/~rc/-rc/ https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz 1.0 uupdate\n",
            ),
        ];
        let fail = vec![
            tree(
                "1.0~rc1-1",
                "version=4\nopts=dversionmangle=s/~rc/-rc/ https://www.example.com/software/foo/ .*/foo-([^-]+)\\.tar\\.gz\n",
            ),
            // The prerelease word is matched in any case.
            tree(
                "2.0~RC1-1",
                "version=4\nopts=versionmangle=s/~RC/-RC/ https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
            ),
        ];
        SourceTreeTester::new(DebianWatchManglesUpstreamVersionImproperly, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "1.0~rc1-1",
            "version=4\nopts=uversionmangle=s/~rc/-rc/ https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "1.0~rc1-1",
            "version=4\nopts=dversionmangle=s/~rc/-rc/ https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
        ))];
        SourcePackageTester::new(DebianWatchManglesUpstreamVersionImproperly, pass, fail)
            .test_and_snapshot();
    }
}
