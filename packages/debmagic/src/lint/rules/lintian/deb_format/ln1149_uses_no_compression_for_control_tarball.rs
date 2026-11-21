use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::deb_members::Tarball;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct UsesNoCompressionForControlTarball;

declare_rule! {
    /// The `.deb`'s Control tarball is a plain `control.tar`, with no
    /// compression extension.
    ///
    /// The Control tarball is the first member after `debian-binary` whose
    /// name does not start with `_`. Only an official name counts
    /// (`control.tar` with an extension from lintian's
    /// `deb-format/control-compressions`, for Debian `gz` and `xz`); any other
    /// name is `malformed-deb-archive`. Reported for debs, udebs and ddebs.
    /// Extra is empty; the Subject is the location.
    UsesNoCompressionForControlTarball,
    code = "LN1149",
    tag = "uses-no-compression-for-control-tarball",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UsesNoCompressionForControlTarball {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(members) = ctx.deb_members() else {
            return;
        };
        if members.official_tarball(Tarball::Control).as_deref() == Some("") {
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
            members(&["debian-binary", "control.tar.gz", "data.tar.xz"]),
            // Not an official name: `malformed-deb-archive`.
            members(&["debian-binary", "control.tar.zst", "data.tar"]),
            members(&["debian-binary", "control.tar.bz2", "data.tar.xz"]),
            // An uncompressed Data tarball is another Tag.
            members(&["debian-binary", "control.tar.xz", "data.tar"]),
            // Not a well-shaped archive.
            members(&["debian-binary", "control.tar"]),
            members(&["control.tar", "debian-binary", "data.tar.xz"]),
        ];
        let fail = vec![
            members(&["debian-binary", "control.tar", "data.tar.xz"]),
            members(&["debian-binary", "_extra", "control.tar", "data.tar.xz"]),
            members(&["debian-binary", "control.tar", "data.tar.xz"]).udeb(),
        ];
        BinaryPackageTester::new(UsesNoCompressionForControlTarball, pass, fail)
            .test_and_snapshot();
    }
}
