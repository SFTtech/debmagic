//! Rules for lintian's `desktop/x11` and `desktop/icons` Checks.
//!
//! Paths under `usr/share/fonts/X11`, `usr/X11R6`, and icon theme directories.

mod dbus_policy;
mod ln0415_package_contains_compiled_font_file;
mod ln0421_icon_size_and_directory_name_mismatch;
mod ln0428_package_installs_file_to_usr_x11r6;
mod ln0631_file_in_discouraged_x11_font_directory;
mod ln0632_file_in_unknown_x11_font_directory;
mod ln0633_desktop_file_in_wrong_dir;
mod ln0844_missing_call_to_update_fonts;
mod ln1043_raster_image_in_scalable_directory;
mod ln1044_package_contains_multiple_dpi_fonts;
mod ln1045_package_mixes_misc_and_dpi_fonts;
mod ln1220_dbus_policy_at_console;

pub(crate) use ln0415_package_contains_compiled_font_file::PackageContainsCompiledFontFile;
pub(crate) use ln0421_icon_size_and_directory_name_mismatch::IconSizeAndDirectoryNameMismatch;
pub(crate) use ln0428_package_installs_file_to_usr_x11r6::PackageInstallsFileToUsrX11r6;
pub(crate) use ln0631_file_in_discouraged_x11_font_directory::FileInDiscouragedX11FontDirectory;
pub(crate) use ln0632_file_in_unknown_x11_font_directory::FileInUnknownX11FontDirectory;
pub(crate) use ln0633_desktop_file_in_wrong_dir::DesktopFileInWrongDir;
pub(crate) use ln0844_missing_call_to_update_fonts::MissingCallToUpdateFonts;
pub(crate) use ln1043_raster_image_in_scalable_directory::RasterImageInScalableDirectory;
pub(crate) use ln1044_package_contains_multiple_dpi_fonts::PackageContainsMultipleDpiFonts;
pub(crate) use ln1045_package_mixes_misc_and_dpi_fonts::PackageMixesMiscAndDpiFonts;
pub(crate) use ln1220_dbus_policy_at_console::DbusPolicyAtConsole;
