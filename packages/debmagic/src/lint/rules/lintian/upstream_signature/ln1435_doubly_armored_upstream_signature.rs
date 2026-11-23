use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::findings;

pub struct DoublyArmoredUpstreamSignature;

declare_rule! {
    /// A detached upstream signature was armored twice, or more, with
    /// `gpg --enarmor`. That is an error: armor the signature once, or
    /// generate it with `gpg --armor --detach-sig`.
    ///
    /// Reported per signature file next to the `.dsc` (of an orig tarball)
    /// whose contents hold an `ARMORED FILE` header and a line that starts
    /// with `LS0tLS1CRUd`, the base64 of an armored block. The extra is the
    /// signature file name. Source package only.
    DoublyArmoredUpstreamSignature,
    code = "LN1435",
    tag = "doubly-armored-upstream-signature",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DoublyArmoredUpstreamSignature {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for finding in findings(ctx, "doubly-armored-upstream-signature") {
            ctx.diagnostic(Diagnostic::error(finding.extra));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, TestCase};

    use super::*;

    const PLAIN: &str =
        "-----BEGIN PGP SIGNATURE-----\n\niQEzBAABCAAdFiEE\n-----END PGP SIGNATURE-----\n";
    const ARMORED: &str = "-----BEGIN PGP ARMORED FILE-----\nVersion: GnuPG\n\niQEzBAABCAAdFiEE\n-----END PGP ARMORED FILE-----\n";
    const DOUBLY: &str = "-----BEGIN PGP ARMORED FILE-----\n\nLS0tLS1CRUdJTiBQR1AgU0lHTkFUVVJFLS0tLS0K\n-----END PGP ARMORED FILE-----\n";

    fn package(signature: &str) -> SourcePackageCase {
        SourcePackageCase::from(TestCase::default()).siblings(vec![
            ("example_1.0.orig.tar.gz", "tarball"),
            ("example_1.0.orig.tar.gz.asc", signature),
        ])
    }

    #[test]
    fn source() {
        let pass = vec![package(PLAIN), package(ARMORED)];
        let fail = vec![package(DOUBLY)];
        SourcePackageTester::new(DoublyArmoredUpstreamSignature, pass, fail).test_and_snapshot();
    }
}
