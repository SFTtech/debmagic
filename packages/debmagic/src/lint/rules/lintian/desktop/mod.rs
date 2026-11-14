//! Rules for lintian's `desktop/x11` and `desktop/icons` Checks.
//!
//! Paths under `usr/share/fonts/X11`, `usr/X11R6`, and icon theme directories.

mod ln0415_package_contains_compiled_font_file;
mod ln0421_icon_size_and_directory_name_mismatch;
mod ln0428_package_installs_file_to_usr_x11r6;
mod ln0631_file_in_discouraged_x11_font_directory;
mod ln0632_file_in_unknown_x11_font_directory;
mod ln0633_desktop_file_in_wrong_dir;

pub(crate) use ln0415_package_contains_compiled_font_file::PackageContainsCompiledFontFile;
pub(crate) use ln0421_icon_size_and_directory_name_mismatch::IconSizeAndDirectoryNameMismatch;
pub(crate) use ln0428_package_installs_file_to_usr_x11r6::PackageInstallsFileToUsrX11r6;
pub(crate) use ln0631_file_in_discouraged_x11_font_directory::FileInDiscouragedX11FontDirectory;
pub(crate) use ln0632_file_in_unknown_x11_font_directory::FileInUnknownX11FontDirectory;
pub(crate) use ln0633_desktop_file_in_wrong_dir::DesktopFileInWrongDir;
