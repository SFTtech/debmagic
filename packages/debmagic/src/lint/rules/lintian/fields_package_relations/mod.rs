//! Rules for lintian's `fields/package-relations` Check.
//!
//! `obsolete_packages` is the obsolete-package and old-Emacs lists. Each Rule
//! owns one Tag.

mod ln0027_package_relation_with_self;
mod ln0107_depends_on_obsolete_package;
mod ln0108_depends_on_old_emacs;
mod obsolete_packages;

pub(crate) use ln0027_package_relation_with_self::PackageRelationWithSelf;
pub(crate) use ln0107_depends_on_obsolete_package::DependsOnObsoletePackage;
pub(crate) use ln0108_depends_on_old_emacs::DependsOnOldEmacs;
