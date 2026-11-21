use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::deb_members::Tarball;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct UdebUsesUnsupportedCompressionForDataTarball;

declare_rule! {
    /// The udeb's Data tarball is not `data.tar.gz` or `data.tar.xz`.
    ///
    /// The Data tarball is the next member after the Control tarball whose
    /// name does not start with `_`. Only an official name is judged (`data.tar`
    /// with an extension from lintian's `deb-format/data-compressions`, for
    /// Debian `gz`, `xz` and `bz2`); any other name is
    /// `malformed-deb-archive`. So an official `data.tar` or `data.tar.bz2`
    /// is reported, `data.tar.gz` and `data.tar.xz` are not. Only for udebs
    /// (`.udeb` file name). Extra is empty; the Subject is the location.
    ///
    /// Native tests only: the one lintian recipe builds its udeb with
    /// `dpkg-deb -Zlzma`, which current dpkg rejects.
    UdebUsesUnsupportedCompressionForDataTarball,
    code = "LN1161",
    tag = "udeb-uses-unsupported-compression-for-data-tarball",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UdebUsesUnsupportedCompressionForDataTarball {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !ctx.is_udeb() {
            return;
        }
        let Some(members) = ctx.deb_members() else {
            return;
        };
        if members
            .official_tarball(Tarball::Data)
            .is_some_and(|extension| extension != "gz" && extension != "xz")
        {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestCase};

    use super::*;

    fn udeb(names: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(TestCase::default())
            .deb_members(names.iter().copied())
            .udeb()
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(TestCase::default()).udeb(),
            udeb(&["debian-binary", "control.tar.xz", "data.tar.gz"]),
            udeb(&["debian-binary", "control.tar.xz", "data.tar.xz"]),
            // Not an official name: `malformed-deb-archive`.
            udeb(&["debian-binary", "control.tar.xz", "data.tar.zst"]),
            udeb(&["debian-binary", "control.tar.xz"]),
            // A deb may use bz2 or no compression.
            BinaryPackageCase::from(TestCase::default()).deb_members([
                "debian-binary",
                "control.tar.xz",
                "data.tar.bz2",
            ]),
            BinaryPackageCase::from(TestCase::default()).deb_members([
                "debian-binary",
                "control.tar.xz",
                "data.tar",
            ]),
        ];
        let fail = vec![
            udeb(&["debian-binary", "control.tar.xz", "data.tar"]),
            udeb(&["debian-binary", "control.tar.xz", "data.tar.bz2"]),
        ];
        BinaryPackageTester::new(UdebUsesUnsupportedCompressionForDataTarball, pass, fail)
            .test_and_snapshot();
    }
}
