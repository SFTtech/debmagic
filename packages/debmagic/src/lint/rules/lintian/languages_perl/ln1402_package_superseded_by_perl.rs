use std::str::FromStr;

use debmagic_common::debian::version::{PackageVersion, apt_compare};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::data;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageSupersededByPerl;

declare_rule! {
    /// A binary package has the name of a Perl core provided package, and its
    /// version is not older than the version the Perl core provides.
    ///
    /// The table is `fields/perl-provides`: a package name and the version in
    /// the Perl core. The comparison uses the Version without its revision,
    /// as lintian does. Extra is `with <core version>`. Binary packages only;
    /// a Source package is never compared. Udebs are checked too.
    PackageSupersededByPerl,
    code = "LN1402",
    tag = "package-superseded-by-perl",
    default_selected = true,
    experimental = false,
}

const PERL_PROVIDES: &str = include_str!("../../../data/fields/perl-provides");

impl BinaryPackageRule for PackageSupersededByPerl {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        let Some(version) = fields.unfolded_value("Version") else {
            return;
        };
        if !is_valid_version(&version) {
            return;
        }
        let Some(name) = fields.get("Package") else {
            return;
        };
        let Some(core_version) = core_version(name) else {
            return;
        };
        // lintian compares against `<epoch>:<upstream>`, without the revision,
        // once Dpkg::Version accepts it; the comparison itself is libapt-pkg's.
        let Ok(own) = PackageVersion::from_str(&version) else {
            return;
        };
        let no_revision = own.epoch_upstream();
        if PackageVersion::from_str(&no_revision).is_err() {
            return;
        }
        if apt_compare(core_version, &no_revision).is_ge() {
            ctx.diagnostic(Diagnostic::warn(format!("with {core_version}")));
        }
    }
}

/// The Perl core version recorded for `name`, if the table lists it.
fn core_version(name: &str) -> Option<&'static str> {
    data::lines(PERL_PROVIDES).find_map(|line| {
        let mut words = line.split_whitespace();
        let (package, version) = (words.next()?, words.next()?);
        (package == name).then_some(version)
    })
}

/// lintian's `Dpkg::Version->is_valid` for a Version, loosely: an optional
/// numeric epoch, an upstream version starting with a digit, and a revision
/// made of the characters dpkg allows.
fn is_valid_version(version: &str) -> bool {
    let rest = match version.split_once(':') {
        Some((epoch, rest)) => {
            if epoch.is_empty() || !epoch.bytes().all(|byte| byte.is_ascii_digit()) {
                return false;
            }
            rest
        }
        None => version,
    };
    let (upstream, revision) = match rest.rsplit_once('-') {
        Some((upstream, revision)) => (upstream, Some(revision)),
        None => (rest, None),
    };
    let upstream_ok = upstream.starts_with(|c: char| c.is_ascii_digit())
        && upstream
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '~' | ':' | '-'));
    let revision_ok = revision.is_none_or(|revision| {
        !revision.is_empty()
            && revision
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '~'))
    });
    upstream_ok && revision_ok
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester};

    use super::*;

    fn package(name: &str, version: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, crate::lint::tester::TestFile)>::new())
            .package(name)
            .field("Version", version)
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Not in the table.
            package("libfoo-perl", "1.0-1"),
            // Newer than the core version.
            package("libsocket-perl", "2.039-1"),
            // An invalid Version is not compared.
            package("libsocket-perl", "x"),
        ];
        let fail = vec![
            // The core version is newer.
            package("libsocket-perl", "2.037-1"),
            // The revision is not compared.
            package("libsocket-perl", "2.038-1"),
            // Equal without revision.
            package("libsocket-perl", "2.038"),
            // Epoch 0 is the same as no epoch.
            package("libsocket-perl", "0:2.038-2"),
            // The core epoch 1 is newer than epoch 0.
            package("libversion-perl", "0.9929-1"),
            // Equal with the same epoch.
            package("libversion-perl", "1:0.9930-3"),
        ];
        BinaryPackageTester::new(PackageSupersededByPerl, pass, fail).test_and_snapshot();
    }
}
