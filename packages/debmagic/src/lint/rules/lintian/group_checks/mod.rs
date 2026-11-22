//! Rules for lintian's `group-checks` Check.

mod ln1441_intra_source_package_circular_dependency;
mod ln1501_dependency_is_not_multi_archified;

pub(crate) use ln1441_intra_source_package_circular_dependency::IntraSourcePackageCircularDependency;
pub(crate) use ln1501_dependency_is_not_multi_archified::DependencyIsNotMultiArchified;
