use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::findings;

pub struct ConcatenatedUpstreamSignatures;

declare_rule! {
    /// A detached upstream signature next to the `.dsc` holds several
    /// concatenated signature blocks. That is likely an error: one signature
    /// block belongs in the file.
    ///
    /// Reported per signature file of an orig tarball (`<tarball>.asc` and
    /// `<uncompressed>.asc`) whose contents contain two or more
    /// `BEGIN PGP SIGNATURE` ... `END PGP SIGNATURE` blocks in a row. The
    /// extra is the signature file name. Source package only: a Source tree
    /// has no sibling files, so it reports nothing.
    ConcatenatedUpstreamSignatures,
    code = "LN1434",
    tag = "concatenated-upstream-signatures",
    default_selected = true,
    experimental = false,
}

impl SourceRule for ConcatenatedUpstreamSignatures {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for finding in findings(ctx, "concatenated-upstream-signatures") {
            ctx.diagnostic(Diagnostic::warn(finding.extra));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestCase};

    use super::*;

    const SIGNATURE: &str =
        "-----BEGIN PGP SIGNATURE-----\n\niQEzBAABCAAdFiEE\n-----END PGP SIGNATURE-----\n";

    fn package(signature: &str) -> SourcePackageCase {
        SourcePackageCase::from(TestCase::default()).siblings(vec![
            ("example_1.0.orig.tar.gz", "tarball"),
            ("example_1.0.orig.tar.gz.asc", signature),
        ])
    }

    #[test]
    fn source() {
        let doubled = format!("{SIGNATURE}{SIGNATURE}");
        let pass = vec![
            package(SIGNATURE),
            // A signature of another file is not checked.
            SourcePackageCase::from(TestCase::default()).siblings(vec![
                ("example_1.0.orig.tar.gz", "t"),
                ("other.asc", doubled.as_str()),
            ]),
        ];
        let fail = vec![
            package(&doubled),
            // A tarball's uncompressed name gives a second signature file.
            SourcePackageCase::from(TestCase::default()).siblings(vec![
                ("example_1.0.orig.tar.gz", "tarball"),
                ("example_1.0.orig.tar.asc", doubled.as_str()),
            ]),
        ];
        SourcePackageTester::new(ConcatenatedUpstreamSignatures, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_tree_has_no_siblings() {
        let tree = TestCase::source_tree([("debian/control", "x")]);
        let pass = vec![tree];
        let fail = Vec::<TestCase>::new();
        SourceTreeTester::new(ConcatenatedUpstreamSignatures, pass, fail).test();
    }
}
