//! Path Rules for files that do not belong in a Debian package.
//!
//! Other platforms' README files, compressed documentation, a loose file in
//! `usr/share/doc`, macOS metadata, AppImages, a non-debug package's
//! `/usr/lib/debug` payload, and linda overrides. Each Rule owns one Tag.
//! Directory locations keep the trailing slash lintian stores.

mod flag;
mod ln0208_package_contains_readme_for_other_platform_or_distro;
mod ln0209_compressed_documentation;
mod ln0210_file_directly_in_usr_share_doc;
mod ln0211_macos_ds_store_file_in_package;
mod ln0212_macos_resource_fork_file_in_package;
mod ln0213_package_installs_appimage;
mod ln0214_debug_suffix_not_dbg;
mod ln0215_package_contains_linda_override;

pub(crate) use ln0208_package_contains_readme_for_other_platform_or_distro::PackageContainsReadmeForOtherPlatformOrDistro;
pub(crate) use ln0209_compressed_documentation::CompressedDocumentation;
pub(crate) use ln0210_file_directly_in_usr_share_doc::FileDirectlyInUsrShareDoc;
pub(crate) use ln0211_macos_ds_store_file_in_package::MacosDsStoreFileInPackage;
pub(crate) use ln0212_macos_resource_fork_file_in_package::MacosResourceForkFileInPackage;
pub(crate) use ln0213_package_installs_appimage::PackageInstallsAppimage;
pub(crate) use ln0214_debug_suffix_not_dbg::DebugSuffixNotDbg;
pub(crate) use ln0215_package_contains_linda_override::PackageContainsLindaOverride;
