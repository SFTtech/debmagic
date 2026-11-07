//! Rules for lintian's `fields/package-relations` Check.
//!
//! `obsolete_packages` is the obsolete-package and old-Emacs lists.
//! `virtual_packages` and `metapackages` are those lintian data files. Each
//! Rule owns one Tag.

/// Relationship fields lintian's obsolete-package and old-Emacs tags read.
pub(super) const RELATION_FIELDS: &[&str] = &["Depends", "Pre-Depends", "Recommends", "Suggests"];

/// Source relationship fields, in the order the Check walks them.
pub(super) const SOURCE_RELATION_FIELDS: &[&str] = &[
    "Build-Depends",
    "Build-Depends-Indep",
    "Build-Depends-Arch",
    "Build-Conflicts",
    "Build-Conflicts-Indep",
    "Build-Conflicts-Arch",
];

/// Build-dependency fields among [`SOURCE_RELATION_FIELDS`].
pub(super) const BUILD_DEPENDS_FIELDS: &[&str] =
    &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

mod ln0027_package_relation_with_self;
mod ln0107_depends_on_obsolete_package;
mod ln0108_depends_on_old_emacs;
mod ln0167_virtual_package_depends_without_real_package_depends;
mod ln0169_build_depends_on_build_essential;
mod ln0170_build_depends_on_1_revision;
mod ln0171_depends_exclusively_on_makedev;
mod ln0173_build_depends_on_obsolete_package;
mod ln0174_build_depends_on_metapackage;
mod ln0445_needless_suggest_recommend_libservlet_java;
mod metapackages;
mod obsolete_packages;
mod virtual_packages;

pub(crate) use ln0027_package_relation_with_self::PackageRelationWithSelf;
pub(crate) use ln0107_depends_on_obsolete_package::DependsOnObsoletePackage;
pub(crate) use ln0108_depends_on_old_emacs::DependsOnOldEmacs;
pub(crate) use ln0167_virtual_package_depends_without_real_package_depends::VirtualPackageDependsWithoutRealPackageDepends;
pub(crate) use ln0169_build_depends_on_build_essential::BuildDependsOnBuildEssential;
pub(crate) use ln0170_build_depends_on_1_revision::BuildDependsOn1Revision;
pub(crate) use ln0171_depends_exclusively_on_makedev::DependsExclusivelyOnMakedev;
pub(crate) use ln0173_build_depends_on_obsolete_package::BuildDependsOnObsoletePackage;
pub(crate) use ln0174_build_depends_on_metapackage::BuildDependsOnMetapackage;
pub(crate) use ln0445_needless_suggest_recommend_libservlet_java::NeedlessSuggestRecommendLibservletJava;
