use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::file::{WatchFileHit, flag_watch_file};

pub struct DebianWatchCouldVerifyDownload;

declare_rule! {
    /// A signing key is shipped and the watch file does not use it.
    ///
    /// Key names are `debian/upstream-signing-key.pgp`,
    /// `debian/upstream/signing-key.pgp`, and
    /// `debian/upstream/signing-key.asc`, first match. Extra is that path.
    /// The watch path is the Location.
    DebianWatchCouldVerifyDownload,
    code = "LN0102",
    tag = "debian-watch-could-verify-download",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianWatchCouldVerifyDownload {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_file(ctx, Severity::Warning, |hit| {
            matches!(hit, WatchFileHit::CouldVerifyDownload(_))
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

    fn tree(watch: &str, key: bool) -> Vec<(String, TestFile)> {
        let mut files = vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            ("debian/watch".to_string(), TestFile::from(watch)),
        ];
        if key {
            files.push((
                "debian/upstream/signing-key.asc".to_string(),
                TestFile::from("key\n"),
            ));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(
                "version=4\nhttps://example.com/foo-(.+)\\.tar\\.gz\n",
                false,
            ),
            tree(
                "version=4\nopts=pgpsigurlmangle=s/$/.asc/ https://example.com/foo-(.+)\\.tar\\.gz\n",
                true,
            ),
        ];
        let fail = vec![tree(
            "version=4\nhttps://example.com/foo-(.+)\\.tar\\.gz\n",
            true,
        )];
        SourceTreeTester::new(DebianWatchCouldVerifyDownload, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "version=4\nhttps://example.com/foo-(.+)\\.tar\\.gz\n",
            false,
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "version=4\nhttps://example.com/foo-(.+)\\.tar\\.gz\n",
            true,
        ))];
        SourcePackageTester::new(DebianWatchCouldVerifyDownload, pass, fail).test_and_snapshot();
    }
}
