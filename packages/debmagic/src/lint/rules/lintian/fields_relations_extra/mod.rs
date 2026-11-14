//! Further `fields/package-relations` Tags, plus the relation Tags of
//! `debian/control/field/relation`, `debian/control/prerequisite/development`,
//! and `debian/control/prerequisite/redundant`, one Rule each.
//!
//! The field lists come from `fields_relations_more`; helpers here flatten a
//! paragraph's relation fields into `(field, alternative)` pairs in the order
//! the Check walks them.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::debian_control::ControlParagraph;
use crate::lint::package_relation::Alternative;

mod ln0560_build_depends_indep_without_arch_indep;
mod ln0561_build_depends_arch_without_arch_dependent_binary;
mod ln0562_conflicting_negation_in_source_relation;
mod ln0563_build_depends_on_versioned_berkeley_db;
mod ln0564_build_depends_on_an_obsolete_java_package;
mod ln0565_build_depends_on_specific_java_doc_package;
mod ln0566_build_depends_on_dh_sysuser;
mod ln0567_build_depends_on_non_build_package;
mod ln0568_bad_version_in_relation;
mod ln0569_obsolete_relation_form_in_source;
mod ln0570_dbg_package_missing_depends;
mod ln0571_weak_library_dev_dependency;
mod ln0572_redundant_installation_prerequisite;
mod ln0573_depends_on_libdb1_compat;
mod ln0574_doc_package_depends_on_main_package;
mod ln0575_depends_on_specific_java_doc_package;
mod ln0576_breaks_without_version;
mod ln0577_default_mta_dependency_does_not_specify_mail_transport_agent;
mod ln0578_default_mta_dependency_not_listed_first;
mod ln0579_binary_package_depends_on_toolchain_package;

pub(crate) use ln0560_build_depends_indep_without_arch_indep::BuildDependsIndepWithoutArchIndep;
pub(crate) use ln0561_build_depends_arch_without_arch_dependent_binary::BuildDependsArchWithoutArchDependentBinary;
pub(crate) use ln0562_conflicting_negation_in_source_relation::ConflictingNegationInSourceRelation;
pub(crate) use ln0563_build_depends_on_versioned_berkeley_db::BuildDependsOnVersionedBerkeleyDb;
pub(crate) use ln0564_build_depends_on_an_obsolete_java_package::BuildDependsOnAnObsoleteJavaPackage;
pub(crate) use ln0565_build_depends_on_specific_java_doc_package::BuildDependsOnSpecificJavaDocPackage;
pub(crate) use ln0566_build_depends_on_dh_sysuser::BuildDependsOnDhSysuser;
pub(crate) use ln0567_build_depends_on_non_build_package::BuildDependsOnNonBuildPackage;
pub(crate) use ln0568_bad_version_in_relation::BadVersionInRelation;
pub(crate) use ln0569_obsolete_relation_form_in_source::ObsoleteRelationFormInSource;
pub(crate) use ln0570_dbg_package_missing_depends::DbgPackageMissingDepends;
pub(crate) use ln0571_weak_library_dev_dependency::WeakLibraryDevDependency;
pub(crate) use ln0572_redundant_installation_prerequisite::RedundantInstallationPrerequisite;
pub(crate) use ln0573_depends_on_libdb1_compat::DependsOnLibdb1Compat;
pub(crate) use ln0574_doc_package_depends_on_main_package::DocPackageDependsOnMainPackage;
pub(crate) use ln0575_depends_on_specific_java_doc_package::DependsOnSpecificJavaDocPackage;
pub(crate) use ln0576_breaks_without_version::BreaksWithoutVersion;
pub(crate) use ln0577_default_mta_dependency_does_not_specify_mail_transport_agent::DefaultMtaDependencyDoesNotSpecifyMailTransportAgent;
pub(crate) use ln0578_default_mta_dependency_not_listed_first::DefaultMtaDependencyNotListedFirst;
pub(crate) use ln0579_binary_package_depends_on_toolchain_package::BinaryPackageDependsOnToolchainPackage;

/// The source fields lintian treats as dependencies, not conflicts.
#[allow(dead_code)]
pub(super) const SOURCE_DEPENDENCY_FIELDS: &[&str] =
    &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

/// Every alternative of the named relation fields, with its field, in the
/// order the fields are named and the alternatives occur.
#[allow(dead_code)]
pub(super) fn alternatives_of(
    fields: &ControlParagraph,
    names: &[&str],
) -> Vec<(String, Alternative)> {
    names
        .iter()
        .filter_map(|field| Some((*field, fields.relation(field)?)))
        .flat_map(|(field, relation)| {
            relation
                .alternatives()
                .map(|alternative| (field.to_string(), alternative.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}

static VIRTUAL_PACKAGES: LazyLock<std::collections::HashSet<String>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/virtual-packages"))
        .map(str::to_string)
        .collect()
});

/// Whether `data/fields/virtual-packages` lists the package.
#[allow(dead_code)]
pub(super) fn is_virtual_package(package: &str) -> bool {
    VIRTUAL_PACKAGES.contains(package)
}

static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("valid regex"));

/// lintian's `trimmed_list($field, qr{\s*,\s*})` of the unfolded value.
#[allow(dead_code)]
pub(super) fn comma_list(fields: &ControlParagraph, field: &str) -> Vec<String> {
    let Some(value) = fields.unfolded_value(field) else {
        return Vec::new();
    };
    let mut items: Vec<&str> = COMMA.split(value.trim()).collect();
    while items.last() == Some(&"") {
        items.pop();
    }
    items
        .into_iter()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether the installable's `Architecture` is exactly `all`.
#[allow(dead_code)]
pub(super) fn is_arch_all(fields: &ControlParagraph) -> bool {
    fields.get("Architecture").unwrap_or_default().trim() == "all"
}

#[cfg(test)]
#[allow(dead_code)]
pub(super) mod testing {
    use crate::lint::tester::{BinaryPackageCase, TestFile};

    /// `debian/control` with the source paragraph fields given and one
    /// arch-all installable `example`.
    pub(in crate::lint::rules::lintian::fields_relations_extra) fn control(
        fields: &[(&str, &str)],
    ) -> Vec<(&'static str, TestFile)> {
        let mut body = String::from("Source: example\n");
        for (name, value) in fields {
            body.push_str(&format!("{name}: {value}\n"));
        }
        body.push_str("\nPackage: example\nArchitecture: all\nDescription: example\n extra\n");
        vec![("debian/control", TestFile::new(body))]
    }

    /// `debian/control` text as given.
    pub(in crate::lint::rules::lintian::fields_relations_extra) fn raw(
        text: &str,
    ) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(text))]
    }

    pub(in crate::lint::rules::lintian::fields_relations_extra) fn binary(
        items: &[(&str, &str)],
    ) -> BinaryPackageCase {
        items.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }
}
