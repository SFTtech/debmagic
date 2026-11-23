use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::findings;

pub struct OrigTarballMissingUpstreamSignature;

declare_rule! {
    /// The packaging ships an upstream signing key, but an orig tarball has
    /// no `.asc` signature next to the `.dsc`. Please add the signature
    /// `<package>_<version>.orig.tar.<ext>.asc` next to the orig tarball.
    ///
    /// Reported per orig tarball (the `.orig.tar.` names, component tarballs
    /// included) that has neither `<tarball>.asc` nor `<uncompressed>.asc`.
    /// Only when a signing key (`debian/upstream/signing-key.asc` and the
    /// other names lintian lists) is in the patched `debian/`, and the
    /// package is neither repacked (upstream version matches `dfsg`,
    /// `debian`, `ds` or `repack`) nor fetched with uscan's `gittag` mode.
    /// The extra is the orig tarball name. Source package only: the sibling
    /// files of a Source tree are not known.
    OrigTarballMissingUpstreamSignature,
    code = "LN1437",
    tag = "orig-tarball-missing-upstream-signature",
    default_selected = true,
    experimental = false,
}

impl SourceRule for OrigTarballMissingUpstreamSignature {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for finding in findings(ctx, "orig-tarball-missing-upstream-signature") {
            ctx.diagnostic(Diagnostic::warn(finding.extra));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, TestCase};

    use super::*;

    const KEY: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\n";

    fn signed_package(version: &str) -> SourcePackageCase {
        let dsc =
            Dsc::parse(&format!("Source: example\nVersion: {version}\n")).expect("dsc paragraph");
        SourcePackageCase::from(vec![("debian/upstream/signing-key.asc", KEY)])
            .siblings(vec![
                ("example_1.0.orig.tar.gz", "tarball"),
                ("example_1.0.orig.tar.gz.asc", "sig"),
            ])
            .dsc(dsc)
    }

    #[test]
    fn source() {
        let unsigned = SourcePackageCase::from(vec![("debian/upstream/signing-key.asc", KEY)])
            .siblings(vec![("example_1.0.orig.tar.gz", "tarball")]);
        // The component tarball needs its own signature.
        let component = SourcePackageCase::from(vec![("debian/upstream/signing-key.asc", KEY)])
            .siblings(vec![
                ("example_1.0.orig.tar.gz", "tarball"),
                ("example_1.0.orig.tar.gz.asc", "sig"),
                ("example_1.0.orig-component.tar.gz", "component"),
            ]);
        // No signing key: nothing to compare with.
        let no_key = SourcePackageCase::from(TestCase::default())
            .siblings(vec![("example_1.0.orig.tar.gz", "tarball")]);
        // A repacked upstream is exempt.
        let repacked = SourcePackageCase::from(vec![("debian/upstream/signing-key.asc", KEY)])
            .siblings(vec![("example_1.0.orig.tar.gz", "tarball")])
            .dsc(Dsc::parse("Source: example\nVersion: 1.0+dfsg-1\n").expect("dsc paragraph"));
        // uscan's gittag mode never matches a signature.
        let gittag = SourcePackageCase::from(vec![
            ("debian/upstream/signing-key.asc", KEY),
            ("debian/watch", "version=4\nopts=pgpmode=gittag\n"),
        ])
        .siblings(vec![("example_1.0.orig.tar.gz", "tarball")]);

        let pass = vec![signed_package("1.0-1"), no_key, repacked, gittag];
        let fail = vec![unsigned, component];
        SourcePackageTester::new(OrigTarballMissingUpstreamSignature, pass, fail)
            .test_and_snapshot();
    }
}
