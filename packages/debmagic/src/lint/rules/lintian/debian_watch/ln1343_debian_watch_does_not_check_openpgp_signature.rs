use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::file::{WatchFileHit, flag_watch_file};

pub struct DebianWatchDoesNotCheckOpenpgpSignature;

declare_rule! {
    /// The watch file does not verify upstream downloads with an OpenPGP
    /// signature.
    ///
    /// Verification is an option `pgpsigurlmangle=` or `pgpmode=` with any
    /// value other than `none`, read from the options of every watch line,
    /// as the watch-file model reads them. Reported once when no line sets
    /// it, with an empty Extra and the watch file as the Location (no line).
    /// Applies to the Source tree and Source package. Experimental, and
    /// pedantic.
    DebianWatchDoesNotCheckOpenpgpSignature,
    code = "LN1343",
    tag = "debian-watch-does-not-check-openpgp-signature",
    default_selected = false,
    experimental = true,
}

impl SourceRule for DebianWatchDoesNotCheckOpenpgpSignature {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_file(ctx, Severity::Pedantic, |hit| {
            matches!(hit, WatchFileHit::DoesNotCheckOpenpgpSignature)
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
            tree(
                "version=4\nopts=pgpsigurlmangle=s/$/.asc/ https://example.com/foo-(.+)\\.tar\\.gz\n",
            ),
            tree("version=4\nopts=pgpmode=auto https://example.com/foo-(.+)\\.tar\\.gz\n"),
            // Not a watch file version: no line is read.
            tree("version=x\nhttps://example.com/foo-(.+)\\.tar\\.gz\n"),
            // No watch file at all.
            vec![("debian/control".to_string(), TestFile::from(CONTROL))],
        ];
        let fail = vec![
            tree("version=4\nhttps://example.com/foo-(.+)\\.tar\\.gz\n"),
            // `pgpmode=none` verifies nothing.
            tree("version=4\nopts=pgpmode=none https://example.com/foo-(.+)\\.tar\\.gz\n"),
        ];
        SourceTreeTester::new(DebianWatchDoesNotCheckOpenpgpSignature, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "version=4\nopts=pgpsigurlmangle=s/$/.asc/ https://example.com/foo-(.+)\\.tar\\.gz\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "version=4\nhttps://example.com/foo-(.+)\\.tar\\.gz\n",
        ))];
        SourcePackageTester::new(DebianWatchDoesNotCheckOpenpgpSignature, pass, fail)
            .test_and_snapshot();
    }
}
