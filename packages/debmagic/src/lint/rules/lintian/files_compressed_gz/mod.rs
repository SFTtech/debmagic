//! Rules for lintian's `files/compressed/gz` Check.

mod ln1069_broken_gz;
mod ln1070_gzip_file_is_not_multi_arch_same_safe;
mod ln1071_package_contains_timestamped_gzip;

pub(crate) use ln1069_broken_gz::BrokenGz;
pub(crate) use ln1070_gzip_file_is_not_multi_arch_same_safe::GzipFileIsNotMultiArchSameSafe;
pub(crate) use ln1071_package_contains_timestamped_gzip::PackageContainsTimestampedGzip;
