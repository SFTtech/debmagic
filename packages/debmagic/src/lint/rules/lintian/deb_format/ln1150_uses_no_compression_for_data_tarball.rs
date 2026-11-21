use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::deb_members::Tarball;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct UsesNoCompressionForDataTarball;

declare_rule! {
    /// The `.deb`'s Data tarball is a plain `data.tar`, with no compression
    /// extension.
    ///
    /// The Data tarball is the next member after the Control tarball whose
    /// name does not start with `_`. Only an official name counts (`data.tar`
    /// with an extension from lintian's `deb-format/data-compressions`, for
    /// Debian `gz`, `xz` and `bz2`); any other name is `malformed-deb-archive`.
    /// A udeb with a plain `data.tar` gets
    /// `udeb-uses-unsupported-compression-for-data-tarball` instead, as in
    /// lintian. Reported for debs and ddebs. Extra is empty; the Subject is
    /// the location.
    UsesNoCompressionForDataTarball,
    code = "LN1150",
    tag = "uses-no-compression-for-data-tarball",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UsesNoCompressionForDataTarball {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(members) = ctx.deb_members() else {
            return;
        };
        if members.official_tarball(Tarball::Data).as_deref() == Some("") {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestCase};

    use super::*;

    fn members(names: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(TestCase::default()).deb_members(names.iter().copied())
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(TestCase::default()),
            members(&["debian-binary", "control.tar.xz", "data.tar.bz2"]),
            // Not an official name: `malformed-deb-archive`.
            members(&["debian-binary", "control.tar.xz", "data.tar.zst"]),
            // An uncompressed Control tarball is another Tag.
            members(&["debian-binary", "control.tar", "data.tar.xz"]),
            // A udeb gets `udeb-uses-unsupported-compression-for-data-tarball`.
            members(&["debian-binary", "control.tar.xz", "data.tar"]).udeb(),
            members(&["debian-binary", "control.tar.xz"]),
        ];
        let fail = vec![
            members(&["debian-binary", "control.tar.xz", "data.tar"]),
            members(&["debian-binary", "control.tar.xz", "_extra", "data.tar"]),
        ];
        BinaryPackageTester::new(UsesNoCompressionForDataTarball, pass, fail).test_and_snapshot();
    }
}
