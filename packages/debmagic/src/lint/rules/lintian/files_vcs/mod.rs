//! Rules for lintian's `files/vcs` Check.

mod ln0034_package_contains_vcs_control_dir;
mod ln0057_svn_commit_file_in_package;
mod ln0128_package_contains_vcs_control_file;
mod ln0134_svk_commit_file_in_package;

pub(crate) use ln0034_package_contains_vcs_control_dir::PackageContainsVcsControlDir;
pub(crate) use ln0057_svn_commit_file_in_package::SvnCommitFileInPackage;
pub(crate) use ln0128_package_contains_vcs_control_file::PackageContainsVcsControlFile;
pub(crate) use ln0134_svk_commit_file_in_package::SvkCommitFileInPackage;
