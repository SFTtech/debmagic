use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::file::{WatchFileHit, flag_watch_file};

pub struct DebianWatchFilePubkeyFileIsMissing;

declare_rule! {
    /// The watch file enables OpenPGP verification and no signing key is
    /// shipped.
    ///
    /// Verification is `pgpsigurlmangle` or `pgpmode` other than `none`. Key
    /// names are `debian/upstream-signing-key.pgp`,
    /// `debian/upstream/signing-key.pgp`, and
    /// `debian/upstream/signing-key.asc`. Extra is empty. The path is the
    /// Location.
    DebianWatchFilePubkeyFileIsMissing,
    code = "LN0101",
    tag = "debian-watch-file-pubkey-file-is-missing",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianWatchFilePubkeyFileIsMissing {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_file(ctx, Severity::Error, |hit| {
            matches!(hit, WatchFileHit::PubkeyFileIsMissing)
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

    const WATCH: &str = "version=4\n\
         opts=pgpsigurlmangle=s/$/.asc/ https://example.com/foo-(.+)\\.tar\\.gz\n";

    fn tree(key: bool) -> Vec<(String, TestFile)> {
        let mut files = vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            ("debian/watch".to_string(), TestFile::from(WATCH)),
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
            tree(true),
            vec![
                ("debian/control".to_string(), TestFile::from(CONTROL)),
                (
                    "debian/watch".to_string(),
                    TestFile::from("version=4\nhttps://example.com/foo-(.+)\\.tar\\.gz\n"),
                ),
            ],
        ];
        let fail = vec![tree(false)];
        SourceTreeTester::new(DebianWatchFilePubkeyFileIsMissing, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(true))];
        let fail = vec![SourcePackageCase::from(tree(false))];
        SourcePackageTester::new(DebianWatchFilePubkeyFileIsMissing, pass, fail)
            .test_and_snapshot();
    }
}
