//! Further `fields/package-relations` Tags, one Rule each.
//!
//! `fields_package_relations` is a separate set of Rules. Shared lists here are
//! the source relationship fields and the binary dependency fields that Check
//! reads, plus the transitional / meta-package skip.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::debian_control::ControlParagraph;

mod architectures;
mod essential;
mod ln0200_invalid_arch_string_in_source_relation;
mod ln0201_depends_on_metapackage;
mod ln0202_depends_on_packaging_dev;
mod ln0203_depends_on_python_minimal;
mod ln0205_needlessly_depends_on_awk;
mod ln0206_package_depends_on_multiple_libstdc_versions;
mod ln0207_build_depends_on_essential_package_without_using_version;
mod ln0260_conflicts_with_dependency;
mod ln0261_mail_transport_agent_dependency_does_not_specify_default_mta;
mod ln0262_build_conflicts_with_build_dependency;
mod ln0263_package_depends_on_multiple_libpng_versions;
mod ln0264_package_depends_on_multiple_tcl_versions;
mod ln0265_package_depends_on_multiple_tk_versions;
mod ln0266_package_relation_with_perl_modules;
mod ln0267_bad_provided_package_name;
mod ln0268_build_depends_on_obsolete_bootstrap;
mod ln0269_depends_on_obsolete_bootstrap;
mod ln0352_build_depends_on_build_essential_package_without_using_version;
mod ln0353_depends_on_essential_package_without_using_version;
mod metapackages;
mod overlap;

pub(crate) use ln0200_invalid_arch_string_in_source_relation::InvalidArchStringInSourceRelation;
pub(crate) use ln0201_depends_on_metapackage::DependsOnMetapackage;
pub(crate) use ln0202_depends_on_packaging_dev::DependsOnPackagingDev;
pub(crate) use ln0203_depends_on_python_minimal::DependsOnPythonMinimal;
pub(crate) use ln0205_needlessly_depends_on_awk::NeedlesslyDependsOnAwk;
pub(crate) use ln0206_package_depends_on_multiple_libstdc_versions::PackageDependsOnMultipleLibstdcVersions;
pub(crate) use ln0207_build_depends_on_essential_package_without_using_version::BuildDependsOnEssentialPackageWithoutUsingVersion;
pub(crate) use ln0260_conflicts_with_dependency::ConflictsWithDependency;
pub(crate) use ln0261_mail_transport_agent_dependency_does_not_specify_default_mta::MailTransportAgentDependencyDoesNotSpecifyDefaultMta;
pub(crate) use ln0262_build_conflicts_with_build_dependency::BuildConflictsWithBuildDependency;
pub(crate) use ln0263_package_depends_on_multiple_libpng_versions::PackageDependsOnMultipleLibpngVersions;
pub(crate) use ln0264_package_depends_on_multiple_tcl_versions::PackageDependsOnMultipleTclVersions;
pub(crate) use ln0265_package_depends_on_multiple_tk_versions::PackageDependsOnMultipleTkVersions;
pub(crate) use ln0266_package_relation_with_perl_modules::PackageRelationWithPerlModules;
pub(crate) use ln0267_bad_provided_package_name::BadProvidedPackageName;
pub(crate) use ln0268_build_depends_on_obsolete_bootstrap::BuildDependsOnObsoleteBootstrap;
pub(crate) use ln0269_depends_on_obsolete_bootstrap::DependsOnObsoleteBootstrap;
pub(crate) use ln0352_build_depends_on_build_essential_package_without_using_version::BuildDependsOnBuildEssentialPackageWithoutUsingVersion;
pub(crate) use ln0353_depends_on_essential_package_without_using_version::DependsOnEssentialPackageWithoutUsingVersion;

/// Source relationship fields the Check walks, in that order.
pub(super) const SOURCE_RELATION_FIELDS: &[&str] = &[
    "Build-Depends",
    "Build-Depends-Indep",
    "Build-Depends-Arch",
    "Build-Conflicts",
    "Build-Conflicts-Indep",
    "Build-Conflicts-Arch",
];

/// Binary fields for which the Check sets `is_dep_field`.
pub(super) const BINARY_DEP_FIELDS: &[&str] = &["Depends", "Pre-Depends", "Recommends", "Suggests"];

/// Binary relationship fields the Check walks, in that order.
pub(super) const BINARY_RELATION_FIELDS: &[&str] = &[
    "Depends",
    "Pre-Depends",
    "Recommends",
    "Suggests",
    "Conflicts",
    "Provides",
    "Enhances",
    "Replaces",
    "Breaks",
];

/// Packages lintian treats as obsolete Bootstrap.
const OBSOLETE_BOOTSTRAP: &[&str] = &[
    "libjs-bootstrap",
    "fonts-glyphicons-halflings",
    "libjs-bootstrap4",
];

/// First alternatives in each dependency field that are in `known`, joined when
/// more than one occurs in a single field.
pub(super) fn repeated_library_versions(fields: &ControlParagraph, known: &[&str]) -> Vec<String> {
    BINARY_DEP_FIELDS
        .iter()
        .filter_map(|field| {
            let relation = fields.relation(field)?;
            let seen: Vec<&str> = relation
                .dependencies()
                .iter()
                .filter_map(|dependency| {
                    let package = dependency.first()?.package();
                    known.contains(&package).then_some(package)
                })
                .collect();
            (seen.len() > 1).then(|| seen.join(" "))
        })
        .collect()
}

/// Alternatives whose package is an obsolete Bootstrap package.
pub(super) fn obsolete_bootstrap_messages(
    fields: &ControlParagraph,
    names: &[&str],
) -> Vec<String> {
    names
        .iter()
        .filter_map(|field| Some((*field, fields.relation(field)?)))
        .flat_map(|(field, relation)| {
            relation
                .alternatives()
                .filter(|alternative| OBSOLETE_BOOTSTRAP.contains(&alternative.package()))
                .map(|alternative| format!("{field} {}", alternative.package()))
                .collect::<Vec<_>>()
        })
        .collect()
}

static TRANSITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)transitional package").expect("valid regex"));
static META_PACKAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)meta[ -]?package|(?:dependency|dummy|empty) package").expect("valid regex")
});
static META_SECTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|/)(?:tasks|metapackages)$").expect("valid regex"));

/// lintian's `is_transitional` or `is_meta_package`.
pub(super) fn is_transitional_or_meta(package: &str, fields: &ControlParagraph) -> bool {
    let description = fields.get("Description").unwrap_or_default();
    let section = fields.get("Section").unwrap_or_default();
    TRANSITIONAL.is_match(description)
        || META_PACKAGE.is_match(description)
        || META_SECTION.is_match(section)
        || package.starts_with("task-")
}
