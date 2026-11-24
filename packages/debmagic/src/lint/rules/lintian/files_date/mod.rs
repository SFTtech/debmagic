//! Rules for lintian's `files/date` Check.

mod ln1382_package_contains_ancient_file;
mod ln1461_orig_tarball_contains_ancient_file;

pub(crate) use ln1382_package_contains_ancient_file::PackageContainsAncientFile;
pub(crate) use ln1461_orig_tarball_contains_ancient_file::OrigTarballContainsAncientFile;
