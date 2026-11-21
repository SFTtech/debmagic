//! Rules for lintian's `mimeinfo` Check.
//!
//! Installed files under `usr/share/mime/`.

mod ln0821_package_contains_mime_file_outside_package_dir;
mod ln0964_package_contains_mime_cache_file;

pub(crate) use ln0821_package_contains_mime_file_outside_package_dir::PackageContainsMimeFileOutsidePackageDir;
pub(crate) use ln0964_package_contains_mime_cache_file::PackageContainsMimeCacheFile;
