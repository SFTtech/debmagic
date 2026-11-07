//! Rules for lintian's `desktop/x11` and `desktop/icons` Checks.
//!
//! Paths under `usr/share/fonts/X11`, `usr/X11R6`, and icon theme directories.

mod ln0415_package_contains_compiled_font_file;
mod ln0421_icon_size_and_directory_name_mismatch;
mod ln0428_package_installs_file_to_usr_x11r6;

pub(crate) use ln0415_package_contains_compiled_font_file::PackageContainsCompiledFontFile;
pub(crate) use ln0421_icon_size_and_directory_name_mismatch::IconSizeAndDirectoryNameMismatch;
pub(crate) use ln0428_package_installs_file_to_usr_x11r6::PackageInstallsFileToUsrX11r6;
