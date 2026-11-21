//! Rules for lintian's `languages/r` Check.

mod ln0729_r_data_without_readme_source;
mod ln1118_r_package_not_arch_all;

pub(crate) use ln0729_r_data_without_readme_source::RDataWithoutReadmeSource;
pub(crate) use ln1118_r_package_not_arch_all::RPackageNotArchAll;
