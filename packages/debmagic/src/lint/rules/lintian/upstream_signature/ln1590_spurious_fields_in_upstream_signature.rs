use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::findings;

pub struct SpuriousFieldsInUpstreamSignature;

declare_rule! {
    /// A detached upstream signature that is armored carries spurious header
    /// fields such as `Comment:` or `Version:`, which older `gpg --enarmor`
    /// versions add. Generate the signature with `gpg --armor --detach-sig`
    /// instead, which adds at most a `Comment:` field.
    ///
    /// Reported per armored signature file next to the `.dsc` (of an orig
    /// tarball) that has a line `Name:` after a newline. The extra is the
    /// signature file name followed by the field names, in file order. Info
    /// severity; not selected by default. Source package only.
    SpuriousFieldsInUpstreamSignature,
    code = "LN1590",
    tag = "spurious-fields-in-upstream-signature",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SpuriousFieldsInUpstreamSignature {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for finding in findings(ctx, "spurious-fields-in-upstream-signature") {
            ctx.diagnostic(Diagnostic::warn(finding.extra).with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, TestCase};

    use super::*;

    const PLAIN: &str =
        "-----BEGIN PGP SIGNATURE-----\n\niQEzBAABCAAdFiEE\n-----END PGP SIGNATURE-----\n";
    const WITH_FIELDS: &str = "-----BEGIN PGP ARMORED FILE-----\nComment: GnuPG v1\nVersion: GnuPG\n\niQEzBAABCAAdFiEE\n-----END PGP ARMORED FILE-----\n";

    fn package(signature: &str) -> SourcePackageCase {
        SourcePackageCase::from(TestCase::default()).siblings(vec![
            ("example_1.0.orig.tar.gz", "tarball"),
            ("example_1.0.orig.tar.gz.asc", signature),
        ])
    }

    #[test]
    fn source() {
        let armored_clean =
            "-----BEGIN PGP ARMORED FILE-----\n\niQEz\n-----END PGP ARMORED FILE-----\n";
        let pass = vec![package(PLAIN), package(armored_clean)];
        let fail = vec![package(WITH_FIELDS)];
        SourcePackageTester::new(SpuriousFieldsInUpstreamSignature, pass, fail).test_and_snapshot();
    }
}
