use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::file::{WatchFileHit, flag_watch_file};

pub struct DebianWatchFileUsesDeprecatedGithubredir;

declare_rule! {
    /// A watch line still uses `githubredir.debian.net`.
    ///
    /// Extra is the line after `opts=` is removed. The path and line are the
    /// Location.
    DebianWatchFileUsesDeprecatedGithubredir,
    code = "LN0096",
    tag = "debian-watch-file-uses-deprecated-githubredir",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianWatchFileUsesDeprecatedGithubredir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_file(ctx, Severity::Error, |hit| {
            matches!(hit, WatchFileHit::DeprecatedGithubredir(_))
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
        let pass = vec![tree(
            "version=4\nhttps://github.com/user/project /(.+)\\.tar\\.gz\n",
        )];
        let fail = vec![tree(
            "version=4\nhttp://githubredir.debian.net/github/user/project /(.+)\\.tar\\.gz\n",
        )];
        SourceTreeTester::new(DebianWatchFileUsesDeprecatedGithubredir, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("version=4\n"))];
        let fail = vec![SourcePackageCase::from(tree(
            "version=4\nhttp://githubredir.debian.net/github/user/project /(.+)\\.tar\\.gz\n",
        ))];
        SourcePackageTester::new(DebianWatchFileUsesDeprecatedGithubredir, pass, fail)
            .test_and_snapshot();
    }
}
