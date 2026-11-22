use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::file::{WatchFileHit, flag_watch_file};

pub struct DebianWatchManglesDebianVersionImproperly;

declare_rule! {
    /// The watch line mangles the upstream version for a repacked version, but
    /// should use `dversionmangle`.
    ///
    /// Fires when the changelog upstream version has a repack marker (`dfsg`,
    /// `debian`, `ds`, or `repack`), the watch line sets a `[ud]?versionmangle`
    /// option for it, and no `dversionmangle` option does. Lintian requires the
    /// line's last version field to be `debian` for watch files before standard
    /// 5, and accepts `auto` or a value with the marker. Extra is the joined
    /// watch line. The file and line are the Location. Info, not selected by
    /// default.
    DebianWatchManglesDebianVersionImproperly,
    code = "LN1537",
    tag = "debian-watch-mangles-debian-version-improperly",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DebianWatchManglesDebianVersionImproperly {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_file(ctx, Severity::Info, |hit| {
            matches!(hit, WatchFileHit::DebianVersionMangledImproperly(_))
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
            // The repack is mangled with dversionmangle.
            tree(
                "1+dfsg-1",
                "version=4\nopts=dversionmangle=s/\\+dfsg$// https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
            ),
            // No repack marker in the upstream version.
            tree(
                "1.0-1",
                "version=4\nopts=uversionmangle=s/$/ds/ https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
            ),
            // A version field other than `debian` keeps the repack out of this check.
            tree(
                "1+dfsg-1",
                "version=4\nopts=uversionmangle=s/$/ds/ https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz 1.0 uupdate\n",
            ),
            // With dversionmangle set to auto, the repack is handled.
            tree(
                "1+dfsg-1",
                "version=4\nopts=dversionmangle=auto,uversionmangle=s/$/ds/ https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
            ),
        ];
        let fail = vec![
            tree(
                "1+dfsg-1",
                "version=4\nopts=\"uversionmangle=s/\\+dfsg$//\" https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
            ),
            tree(
                "1+ds-1",
                "version=4\nopts=uversionmangle=s/$/ds/ https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
            ),
        ];
        SourceTreeTester::new(DebianWatchManglesDebianVersionImproperly, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "1+dfsg-1",
            "version=4\nopts=dversionmangle=s/\\+dfsg$// https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "1+dfsg-1",
            "version=4\nopts=uversionmangle=s/\\+dfsg$// https://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
        ))];
        SourcePackageTester::new(DebianWatchManglesDebianVersionImproperly, pass, fail)
            .test_and_snapshot();
    }
}
