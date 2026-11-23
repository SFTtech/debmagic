use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::findings;

pub struct ExplicitlyArmoredUpstreamSignature;

declare_rule! {
    /// A detached upstream signature was armored explicitly with
    /// `gpg --enarmor`. That is likely an error: generate the signature with
    /// `gpg --armor --detach-sig` instead.
    ///
    /// Reported per signature file next to the `.dsc` (of an orig tarball)
    /// whose contents hold an `ARMORED FILE` header that is not the doubly
    /// armored form (`LS0tLS1CRUd`, which `DoublyArmoredUpstreamSignature`
    /// reports). The extra is the signature file name. Source package only.
    ExplicitlyArmoredUpstreamSignature,
    code = "LN1436",
    tag = "explicitly-armored-upstream-signature",
    default_selected = true,
    experimental = false,
}

impl SourceRule for ExplicitlyArmoredUpstreamSignature {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for finding in findings(ctx, "explicitly-armored-upstream-signature") {
            ctx.diagnostic(Diagnostic::warn(finding.extra));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, TestCase};

    use super::*;

    const PLAIN: &str =
        "-----BEGIN PGP SIGNATURE-----\n\niQEzBAABCAAdFiEE\n-----END PGP SIGNATURE-----\n";
    const ARMORED: &str =
        "-----BEGIN PGP ARMORED FILE-----\n\niQEzBAABCAAdFiEE\n-----END PGP ARMORED FILE-----\n";
    const DOUBLY: &str = "-----BEGIN PGP ARMORED FILE-----\n\nLS0tLS1CRUdJTiBQR1AgU0lHTkFUVVJFLS0tLS0K\n-----END PGP ARMORED FILE-----\n";

    fn package(signature: &str) -> SourcePackageCase {
        SourcePackageCase::from(TestCase::default()).siblings(vec![
            ("example_1.0.orig.tar.gz", "tarball"),
            ("example_1.0.orig.tar.gz.asc", signature),
        ])
    }

    #[test]
    fn source() {
        let pass = vec![package(PLAIN), package(DOUBLY)];
        let fail = vec![package(ARMORED)];
        SourcePackageTester::new(ExplicitlyArmoredUpstreamSignature, pass, fail)
            .test_and_snapshot();
    }
}
