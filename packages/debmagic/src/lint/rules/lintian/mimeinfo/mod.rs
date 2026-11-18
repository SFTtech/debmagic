//! Rules for lintian's `mimeinfo` Check.
//!
//! Installed files under `usr/share/mime/`.

mod ln0821_package_contains_mime_file_outside_package_dir;

pub(crate) use ln0821_package_contains_mime_file_outside_package_dir::PackageContainsMimeFileOutsidePackageDir;
