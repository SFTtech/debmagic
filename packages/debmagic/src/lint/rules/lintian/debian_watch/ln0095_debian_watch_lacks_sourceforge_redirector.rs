use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::file::{WatchFileHit, flag_watch_file};

pub struct DebianWatchLacksSourceforgeRedirector;

declare_rule! {
    /// A watch URL still points at a SourceForge download host.
    ///
    /// Matches `dl` / `download` / `ftp` / `upload` on `sourceforge.net` or
    /// `sf.net`, `showfiles.php`, and `/projects/…/files`. Extra is the line
    /// after `opts=` is removed. The path and line are the Location.
    DebianWatchLacksSourceforgeRedirector,
    code = "LN0095",
    tag = "debian-watch-lacks-sourceforge-redirector",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianWatchLacksSourceforgeRedirector {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_file(ctx, Severity::Warning, |hit| {
            matches!(hit, WatchFileHit::LacksSourceforgeRedirector(_))
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

    fn tree(watch: &str) -> Vec<(String, TestFile)> {
        vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            ("debian/watch".to_string(), TestFile::from(watch)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("version=4\nhttps://example.com/foo-(.+)\\.tar\\.gz\n"),
            tree("version=4\nhttp://sf.net/uses/redirector-(.+)\\.tar\\.gz\n"),
        ];
        let fail = vec![
            tree("version=4\nhttp://ftp.sf.net/foo/foo-(.+)\\.tar\\.gz\n"),
            tree("version=4\nhttp://sourceforge.net/projects/synce/files foo-(.+)\\.tar\\.gz\n"),
        ];
        SourceTreeTester::new(DebianWatchLacksSourceforgeRedirector, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "version=4\nhttps://example.com/foo-(.+)\\.tar\\.gz\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "version=4\nftp://upload.sourceforge.net/pub/foo foo-(.+)\\.tar\\.gz\n",
        ))];
        SourcePackageTester::new(DebianWatchLacksSourceforgeRedirector, pass, fail)
            .test_and_snapshot();
    }
}
