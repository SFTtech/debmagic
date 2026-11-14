//! Whether a relation implies an unversioned package, or an OR of them.
//!
//! This is the fragment of lintian's `Relation::satisfies` these Tags need.
//! A dependency implies the target when every alternative names one of the
//! packages, with no architecture restriction, architecture qualifier, or
//! build profile. A version constraint still implies the unversioned name.
//! The relation implies the target when any of its dependencies does.
//! `PackageRelation` has no `satisfies` method; this stays in the Check.

use crate::lint::context::BinaryPackageContext;
use crate::lint::package_relation::{Alternative, PackageRelation};

/// Packages that provide debconf, OR'd, from the Check's `$ANY_DEBCONF`.
const ANY_DEBCONF: &[&str] = &[
    "debconf",
    "debconf-2.0",
    "cdebconf",
    "cdebconf-udeb",
    "libdebconfclient0",
    "libdebconfclient0-udeb",
];

pub(super) fn pre_depends_on_debconf(ctx: &BinaryPackageContext<'_>) -> bool {
    ctx.installation_control()
        .fields()
        .relation("Pre-Depends")
        .is_some_and(|relation| implies(&relation, ANY_DEBCONF))
}

/// `Pre-Depends`, `Depends`, `Provides`, or the package's own name implies
/// `dbconfig-common`. That is lintian's strong dependencies AND Provides AND
/// the package itself, which is what allows a `config` script with no
/// templates.
pub(super) fn uses_dbconfig(ctx: &BinaryPackageContext<'_>) -> bool {
    if ctx.package_name() == "dbconfig-common" {
        return true;
    }
    let fields = ctx.installation_control().fields();
    ["Pre-Depends", "Depends", "Provides"]
        .into_iter()
        .any(|name| {
            fields
                .relation(name)
                .is_some_and(|relation| implies(&relation, &["dbconfig-common"]))
        })
}

fn implies(relation: &PackageRelation, targets: &[&str]) -> bool {
    relation.dependencies().iter().any(|dependency| {
        if dependency.is_empty() {
            return false;
        }
        if dependency.len() == 1 {
            predicate_implies(&dependency[0], targets)
        } else {
            dependency
                .iter()
                .all(|alternative| predicate_implies(alternative, targets))
        }
    })
}

fn predicate_implies(alternative: &Alternative, targets: &[&str]) -> bool {
    if !alternative.rest().is_empty()
        || !alternative.architecture_qualifier().is_empty()
        || !alternative.architectures().is_empty()
        || !alternative.profiles().is_empty()
    {
        return false;
    }
    targets.contains(&alternative.package())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn or_of_providers_and_version_constraints() {
        assert!(implies(&PackageRelation::parse("debconf"), ANY_DEBCONF));
        assert!(implies(
            &PackageRelation::parse("debconf (>= 1.5.0)"),
            ANY_DEBCONF
        ));
        assert!(implies(
            &PackageRelation::parse("debconf | cdebconf"),
            ANY_DEBCONF
        ));
        assert!(implies(
            &PackageRelation::parse("something, libdebconfclient0"),
            ANY_DEBCONF
        ));
        assert!(!implies(
            &PackageRelation::parse("debconf | mail-transport-agent"),
            ANY_DEBCONF
        ));
        assert!(!implies(
            &PackageRelation::parse("debconf [amd64]"),
            ANY_DEBCONF
        ));
        assert!(!implies(
            &PackageRelation::parse("debconf:any"),
            ANY_DEBCONF
        ));
        assert!(!implies(&PackageRelation::parse(""), ANY_DEBCONF));
        assert!(!implies(
            &PackageRelation::parse("${misc:Pre-Depends}"),
            ANY_DEBCONF
        ));
    }
}
