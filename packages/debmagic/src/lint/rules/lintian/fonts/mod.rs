//! Rules for lintian's `fonts` Check.

mod ln1088_duplicate_font_file;
mod ln1089_font_in_non_font_package;
mod ln1090_woff_font_in_wrong_dir;
mod ln1392_font_outside_font_dir;

pub(crate) use ln1088_duplicate_font_file::DuplicateFontFile;
pub(crate) use ln1089_font_in_non_font_package::FontInNonFontPackage;
pub(crate) use ln1090_woff_font_in_wrong_dir::WoffFontInWrongDir;
pub(crate) use ln1392_font_outside_font_dir::FontOutsideFontDir;
