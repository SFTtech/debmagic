//! Rules for lintian's `languages/php/embedded` Check.

mod ln0652_embedded_php_library;
mod ln0820_obsolete_comments_style_in_php_ini;
mod ln1112_composer_prerequisite;
mod ln1282_missing_pkg_php_tools_addon;
mod pear;

pub(crate) use ln0652_embedded_php_library::EmbeddedPhpLibrary;
pub(crate) use ln0820_obsolete_comments_style_in_php_ini::ObsoleteCommentsStyleInPhpIni;
pub(crate) use ln1112_composer_prerequisite::ComposerPrerequisite;
pub(crate) use ln1282_missing_pkg_php_tools_addon::MissingPkgPhpToolsAddon;
