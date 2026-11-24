use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::file::{WatchFileHit, flag_watch_file};

pub struct DebianWatchLineInvalid;

declare_rule! {
    /// A `debian/watch` line the version parser rejects.
    ///
    /// A `version=` of 1–4 and a `version` / `Version` of 5 or newer are
    /// accepted. `Version: 4` and `version=0` are not. Extra is the line. The
    /// path and line are the Location. A standard below 2 stops the walk.
    DebianWatchLineInvalid,
    code = "LN0094",
    tag = "debian-watch-line-invalid",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianWatchLineInvalid {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_file(ctx, Severity::Warning, |hit| {
            matches!(hit, WatchFileHit::LineInvalid(_))
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
            tree("version=5\nhttps://example.com/foo-(.+)\\.tar\\.gz\n"),
            tree("# only a comment\n"),
        ];
        let fail = vec![
            tree("version=0\n"),
            tree("Version: 4\nhttps://example.com/foo-(.+)\\.tar\\.gz\n"),
            tree(
                "version=3\nopts=pgpsigurlmangle=s/$/.asc/ \nhttps://example.com/foo-(.+)\\.tar\\.gz\n",
            ),
        ];
        SourceTreeTester::new(DebianWatchLineInvalid, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("version=4\n"))];
        let fail = vec![SourcePackageCase::from(tree("version=0\n"))];
        SourcePackageTester::new(DebianWatchLineInvalid, pass, fail).test_and_snapshot();
    }
}
