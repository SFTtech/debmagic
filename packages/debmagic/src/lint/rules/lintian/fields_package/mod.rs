//! Rules for lintian's `fields/package` Check.

mod ln1059_bad_package_name;
mod ln1060_package_not_lowercase;
mod ln1671_unusual_documentation_package_name;

pub(crate) use ln1059_bad_package_name::BadPackageName;
pub(crate) use ln1060_package_not_lowercase::PackageNotLowercase;
pub(crate) use ln1671_unusual_documentation_package_name::UnusualDocumentationPackageName;
