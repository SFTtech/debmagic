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
mod metapackages;

pub(crate) use ln0200_invalid_arch_string_in_source_relation::InvalidArchStringInSourceRelation;
pub(crate) use ln0201_depends_on_metapackage::DependsOnMetapackage;
pub(crate) use ln0202_depends_on_packaging_dev::DependsOnPackagingDev;
pub(crate) use ln0203_depends_on_python_minimal::DependsOnPythonMinimal;
pub(crate) use ln0205_needlessly_depends_on_awk::NeedlesslyDependsOnAwk;
pub(crate) use ln0206_package_depends_on_multiple_libstdc_versions::PackageDependsOnMultipleLibstdcVersions;
pub(crate) use ln0207_build_depends_on_essential_package_without_using_version::BuildDependsOnEssentialPackageWithoutUsingVersion;

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
