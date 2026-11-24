use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::standard::{WatchHit, flag_watch};

pub struct MissingDebianWatchFileStandard;

declare_rule! {
    /// `debian/watch` has content and no `version=` or `Version:` line.
    ///
    /// An empty file and a comment-only file are left alone. When this fires,
    /// the other watch-standard Tags do not. Extra is empty; the path is the
    /// Location.
    MissingDebianWatchFileStandard,
    code = "LN0089",
    tag = "missing-debian-watch-file-standard",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MissingDebianWatchFileStandard {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch(ctx, |hit| match hit {
            WatchHit::Missing => Some(String::new()),
            _ => None,
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
            vec![("debian/control".to_string(), TestFile::from(CONTROL))],
            tree(""),
            tree("# comment only\n"),
            tree("version=4\nhttp://example.com\n"),
            vec![
                ("debian/control".to_string(), TestFile::from(CONTROL)),
                ("debian/watch".to_string(), TestFile::directory()),
            ],
            vec![
                ("debian/control".to_string(), TestFile::from(CONTROL)),
                ("debian/watch".to_string(), TestFile::symlink("elsewhere")),
            ],
        ];
        let fail = vec![tree("http://example.com/dist file.tar.gz\n"), tree("\n")];
        SourceTreeTester::new(MissingDebianWatchFileStandard, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/watch".to_string(),
            TestFile::from("version=5\n"),
        )])];
        let fail = vec![SourcePackageCase::from(vec![(
            "debian/watch".to_string(),
            TestFile::from("http://example.com\n"),
        )])];
        SourcePackageTester::new(MissingDebianWatchFileStandard, pass, fail).test_and_snapshot();
    }
}
