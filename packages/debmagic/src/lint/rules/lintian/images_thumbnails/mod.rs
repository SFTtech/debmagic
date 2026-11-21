//! Rules for lintian's `images/thumbnails` Check.

mod ln1094_package_contains_thumbnails_dir;
mod ln1095_package_contains_xvpics_dir;

pub(crate) use ln1094_package_contains_thumbnails_dir::PackageContainsThumbnailsDir;
pub(crate) use ln1095_package_contains_xvpics_dir::PackageContainsXvpicsDir;
